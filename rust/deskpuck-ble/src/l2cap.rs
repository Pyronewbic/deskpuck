//! A direct ATT link to a Joy-Con 2 over an L2CAP LE socket, for Linux: the
//! LE link comes up under BlueZ, but BlueZ never finishes resolving the
//! Joy-Con 2's services, so its GATT API cannot be used. The socket needs no
//! privileges at low security; the `att::Client` does the protocol.

use crate::att::{Client, Event};
use std::io;
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};
use std::time::{Duration, Instant};
use tokio::io::unix::AsyncFd;
use tokio::sync::mpsc::UnboundedReceiver;

const BTPROTO_L2CAP: libc::c_int = 0;
const SOL_BLUETOOTH: libc::c_int = 274;
const BT_SECURITY: libc::c_int = 4;
const BT_SECURITY_LOW: u8 = 1;
/// The fixed L2CAP channel ATT uses on LE.
const ATT_CID: u16 = 4;
const BDADDR_LE_PUBLIC: u8 = 1;
const BDADDR_LE_RANDOM: u8 = 2;

/// `struct sockaddr_l2` from the kernel's Bluetooth headers.
#[repr(C)]
struct SockaddrL2 {
    family: libc::sa_family_t,
    psm: u16,
    /// Least significant byte first.
    bdaddr: [u8; 6],
    cid: u16,
    bdaddr_type: u8,
}

/// `struct bt_security`.
#[repr(C)]
struct BtSecurity {
    level: u8,
    key_size: u8,
}

fn check(result: libc::c_int) -> io::Result<libc::c_int> {
    if result < 0 { Err(io::Error::last_os_error()) } else { Ok(result) }
}

fn address(bdaddr: [u8; 6], bdaddr_type: u8) -> SockaddrL2 {
    SockaddrL2 {
        family: libc::AF_BLUETOOTH as libc::sa_family_t,
        psm: 0,
        bdaddr,
        cid: ATT_CID.to_le(),
        bdaddr_type,
    }
}

fn set_low_security(fd: &OwnedFd) -> io::Result<()> {
    let security = BtSecurity { level: BT_SECURITY_LOW, key_size: 0 };
    // SAFETY: the option value is a live `bt_security` of the size passed.
    check(unsafe {
        libc::setsockopt(
            fd.as_raw_fd(),
            SOL_BLUETOOTH,
            BT_SECURITY,
            (&raw const security).cast(),
            size_of::<BtSecurity>() as libc::socklen_t,
        )
    })
    .map(drop)
}

/// Connects to `device` (most significant byte first, as displayed) on the
/// ATT channel. Waits as long as the kernel tries; the caller bounds it.
pub async fn connect(device: [u8; 6], random: bool) -> io::Result<AsyncFd<OwnedFd>> {
    // SAFETY: plain socket creation; the descriptor is owned right away.
    let raw = check(unsafe {
        libc::socket(
            libc::AF_BLUETOOTH,
            libc::SOCK_SEQPACKET | libc::SOCK_NONBLOCK | libc::SOCK_CLOEXEC,
            BTPROTO_L2CAP,
        )
    })?;
    // SAFETY: `raw` is a new descriptor nothing else owns.
    let fd = unsafe { OwnedFd::from_raw_fd(raw) };
    set_low_security(&fd)?;

    let local = address([0; 6], BDADDR_LE_PUBLIC);
    let mut remote_address = device;
    remote_address.reverse();
    let remote = address(remote_address, if random { BDADDR_LE_RANDOM } else { BDADDR_LE_PUBLIC });
    let len = size_of::<SockaddrL2>() as libc::socklen_t;
    // SAFETY: both addresses are live `sockaddr_l2`s of the length passed.
    check(unsafe { libc::bind(fd.as_raw_fd(), (&raw const local).cast(), len) })?;
    // SAFETY: as above.
    let started = unsafe { libc::connect(fd.as_raw_fd(), (&raw const remote).cast(), len) };
    if started < 0 {
        let e = io::Error::last_os_error();
        if e.raw_os_error() != Some(libc::EINPROGRESS) {
            return Err(e);
        }
    }

    let fd = AsyncFd::new(fd)?;
    let _ready = fd.writable().await?;
    let mut error: libc::c_int = 0;
    let mut error_len = size_of::<libc::c_int>() as libc::socklen_t;
    // SAFETY: `error` and `error_len` are live and sized for SO_ERROR.
    check(unsafe {
        libc::getsockopt(
            fd.as_raw_fd(),
            libc::SOL_SOCKET,
            libc::SO_ERROR,
            (&raw mut error).cast(),
            &raw mut error_len,
        )
    })?;
    if error != 0 {
        return Err(io::Error::from_raw_os_error(error));
    }
    Ok(fd)
}

/// What the controller asks of a running link.
#[derive(Debug)]
pub enum Command {
    Discover,
    Subscribe,
    Write(Vec<u8>),
}

async fn send(fd: &AsyncFd<OwnedFd>, pdu: &[u8]) -> io::Result<()> {
    loop {
        let mut ready = fd.writable().await?;
        // SAFETY: `pdu` is a live buffer of the length passed.
        let sent = ready.try_io(|fd| {
            check(unsafe {
                libc::send(
                    fd.as_raw_fd(),
                    pdu.as_ptr().cast(),
                    pdu.len(),
                    libc::MSG_DONTWAIT | libc::MSG_NOSIGNAL,
                ) as libc::c_int
            })
        });
        match sent {
            Ok(result) => return result.map(drop),
            Err(_would_block) => continue,
        }
    }
}

/// Carries PDUs between the socket and `client` until the link closes,
/// `commands` ends, or a request times out. Returns why it stopped.
pub async fn run(
    fd: AsyncFd<OwnedFd>,
    mut client: Client,
    mut commands: UnboundedReceiver<Command>,
    mut deliver: impl FnMut(Event),
) -> String {
    let start = Instant::now();
    let now = || start.elapsed().as_secs_f64();
    // Larger than the largest ATT PDU, so a packet is never cut short.
    let mut buf = [0u8; 1024];
    loop {
        for pdu in client.take_outgoing() {
            if let Err(e) = send(&fd, &pdu).await {
                return format!("send failed: {e}");
            }
        }
        if let Some(why) = client.failed() {
            return why.to_owned();
        }
        let wake = client.deadline().map_or(Duration::from_secs(3600), |at| {
            Duration::from_secs_f64(at).saturating_sub(start.elapsed())
        });
        let events = tokio::select! {
            ready = fd.readable() => {
                let mut ready = match ready {
                    Ok(ready) => ready,
                    Err(e) => return format!("socket error: {e}"),
                };
                // SAFETY: `buf` is a live buffer of the length passed.
                let read = ready.try_io(|fd| {
                    check(unsafe {
                        libc::recv(fd.as_raw_fd(), buf.as_mut_ptr().cast(), buf.len(), libc::MSG_DONTWAIT)
                            as libc::c_int
                    })
                });
                match read {
                    Ok(Ok(0)) => return "the Joy-Con closed the link".into(),
                    Ok(Ok(n)) => client.receive(&buf[..n as usize], now()),
                    Ok(Err(e)) => return format!("receive failed: {e}"),
                    Err(_would_block) => Vec::new(),
                }
            }
            command = commands.recv() => match command {
                Some(Command::Discover) => {
                    client.discover(now());
                    Vec::new()
                }
                Some(Command::Subscribe) => client.subscribe(now()),
                Some(Command::Write(data)) => client.write(&data),
                None => return "closed by Deskpuck".into(),
            },
            () = tokio::time::sleep(wake) => {
                client.tick(now());
                Vec::new()
            }
        };
        events.into_iter().for_each(&mut deliver);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::att::{REQUEST_TIMEOUT, uuid_le};
    use crate::receiver::{NOTIFY_CHARACTERISTIC, SERVICE, WRITE_CHARACTERISTIC};
    use std::io::{Read, Write};
    use std::os::unix::net::UnixStream;
    use tokio::sync::mpsc::unbounded_channel;

    fn uuid(s: &str) -> Vec<u8> {
        uuid_le(s).unwrap().to_vec()
    }

    fn client() -> Client {
        let u = |s| uuid_le(s).unwrap();
        Client::new(u(SERVICE), u(NOTIFY_CHARACTERISTIC), u(WRITE_CHARACTERISTIC))
    }

    /// A connected SEQPACKET pair: the link's end, and a blocking end that
    /// plays the Joy-Con.
    fn pair() -> (AsyncFd<OwnedFd>, UnixStream) {
        let mut fds = [0; 2];
        // SAFETY: `fds` has room for the two descriptors socketpair writes.
        let made = unsafe {
            libc::socketpair(
                libc::AF_UNIX,
                libc::SOCK_SEQPACKET | libc::SOCK_CLOEXEC,
                0,
                fds.as_mut_ptr(),
            )
        };
        check(made).unwrap();
        // SAFETY: both descriptors are new and owned by nothing else.
        let (link, joycon) =
            unsafe { (OwnedFd::from_raw_fd(fds[0]), UnixStream::from_raw_fd(fds[1])) };
        // SAFETY: setting O_NONBLOCK on a descriptor this test owns.
        unsafe { libc::fcntl(link.as_raw_fd(), libc::F_SETFL, libc::O_NONBLOCK) };
        (AsyncFd::new(link).unwrap(), joycon)
    }

    fn expect(joycon: &mut UnixStream, want: &[u8], answer: &[u8]) {
        let mut buf = [0u8; 600];
        let n = joycon.read(&mut buf).unwrap();
        assert_eq!(&buf[..n], want);
        joycon.write_all(answer).unwrap();
    }

    #[tokio::test]
    async fn discovers_subscribes_writes_and_streams_over_a_socket() {
        let (link, mut joycon) = pair();
        let (commands, rx) = unbounded_channel();
        let joycon = std::thread::spawn(move || {
            expect(&mut joycon, &[0x02, 0x05, 0x02], &[0x03, 0x00, 0x02]);
            let services = [vec![0x11, 20, 0x08, 0x00, 0x2A, 0x00], uuid(SERVICE)].concat();
            expect(&mut joycon, &[0x10, 0x01, 0x00, 0xFF, 0xFF, 0x00, 0x28], &services);
            let notify = [vec![0x09, 0x00, 0x12, 0x0A, 0x00], uuid(NOTIFY_CHARACTERISTIC)].concat();
            let write = [vec![0x13, 0x00, 0x04, 0x14, 0x00], uuid(WRITE_CHARACTERISTIC)].concat();
            let characteristics = [vec![0x09, 21], notify, write].concat();
            expect(&mut joycon, &[0x08, 0x08, 0x00, 0x2A, 0x00, 0x03, 0x28], &characteristics);
            // Characteristics are read until the Joy-Con says there are no more.
            let none_left = [0x01, 0x08, 0x14, 0x00, 0x0A];
            expect(&mut joycon, &[0x08, 0x14, 0x00, 0x2A, 0x00, 0x03, 0x28], &none_left);
            expect(
                &mut joycon,
                &[0x04, 0x0B, 0x00, 0x12, 0x00],
                &[0x05, 0x01, 0x0B, 0x00, 0x02, 0x29],
            );
            expect(&mut joycon, &[0x12, 0x0B, 0x00, 0x01, 0x00], &[0x13]);
            expect(&mut joycon, &[0x52, 0x14, 0x00, 0x0C, 0x91], &[0x1B, 0x0A, 0x00, 7, 8, 9]);
            // Closing ends the link.
        });
        let mut events = Vec::new();
        let commands_for_events = commands.clone();
        let why = run(link, client(), rx, |event| {
            match &event {
                Event::Discovered { .. } => {
                    commands_for_events.send(Command::Subscribe).unwrap();
                }
                Event::Log(m) if m == "notifications enabled" => {
                    commands_for_events.send(Command::Write(vec![0x0C, 0x91])).unwrap();
                }
                _ => {}
            }
            events.push(event);
        });
        commands.send(Command::Discover).unwrap();
        let why = why.await;
        joycon.join().unwrap();
        assert_eq!(why, "the Joy-Con closed the link");
        assert!(events.contains(&Event::Discovered { write: true, notify: true }), "{events:?}");
        assert_eq!(events.last(), Some(&Event::Notification(vec![7, 8, 9])));
    }

    #[tokio::test]
    async fn an_unanswered_request_ends_the_link() {
        let (link, mut joycon) = pair();
        let (commands, rx) = unbounded_channel();
        commands.send(Command::Discover).unwrap();
        let started = Instant::now();
        let why = run(link, client(), rx, |_| {}).await;
        assert_eq!(why, "ATT request 0x02 timed out");
        assert!(started.elapsed().as_secs_f64() >= REQUEST_TIMEOUT);
        let mut buf = [0u8; 16];
        assert_eq!(joycon.read(&mut buf).unwrap(), 3, "control: the request was sent");
    }

    #[tokio::test]
    async fn dropping_the_commands_closes_the_link() {
        let (link, mut joycon) = pair();
        let (commands, rx) = unbounded_channel();
        drop(commands);
        assert_eq!(run(link, client(), rx, |_| {}).await, "closed by Deskpuck");
        let mut buf = [0u8; 16];
        assert_eq!(joycon.read(&mut buf).unwrap(), 0, "the socket was closed");
    }

    #[test]
    fn the_socket_address_matches_the_kernel_layout() {
        assert_eq!(size_of::<SockaddrL2>(), 14);
        assert_eq!(size_of::<BtSecurity>(), 2);
        assert_eq!(std::mem::offset_of!(SockaddrL2, bdaddr), 4);
        assert_eq!(std::mem::offset_of!(SockaddrL2, cid), 10);
        assert_eq!(std::mem::offset_of!(SockaddrL2, bdaddr_type), 12);
        let a = address([0x72, 0x2B, 0x2A, 0xBF, 0xEF, 0xE0], BDADDR_LE_PUBLIC);
        assert_eq!(a.family, libc::AF_BLUETOOTH as libc::sa_family_t);
        assert_eq!(a.psm, 0);
        assert_eq!(a.cid.to_ne_bytes(), [4, 0], "little-endian on the wire");
        assert_eq!(a.bdaddr_type, 1);
    }
}
