//! The Joy-Con 2's own pairing (command 0x15), after which it reconnects to this computer on a
//! button press. Bytes only, no I/O; every reply is attacker bytes and is length-checked.

use aes::Aes128;
use aes::cipher::{Array, BlockCipherEncrypt, KeyInit};
use deskpuck_core::pairing::HostAddress;
use std::fmt;

/// Replies to command 0x15 arrive here, not on the input characteristic.
pub const RESPONSE_CHARACTERISTIC: &str = "c765a961-d9d8-4d36-a20a-5315b111836a";

/// Whom a Joy-Con 2's advertisement is for.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Addressee {
    /// Too short, or not Nintendo's: decide as if the field were absent.
    #[default]
    Unknown,
    /// SYNC mode: any host may connect.
    Anyone,
    /// After a button press: the host it was vendor-paired to, the only one it accepts.
    Host(HostAddress),
}

/// From manufacturer data as btleplug gives it, without the company id: the vendor id at 3,
/// then the host address least significant byte first at 10..16, all zeros in SYNC mode.
pub fn addressee(data: &[u8]) -> Addressee {
    let (Some(vendor), Some(host)) = (data.get(3..5), data.get(10..16)) else {
        return Addressee::Unknown;
    };
    if vendor != [0x7E, 0x05] {
        return Addressee::Unknown;
    }
    let mut wire = [0u8; 6];
    wire.copy_from_slice(host);
    HostAddress::from_wire(wire).map_or(Addressee::Anyone, Addressee::Host)
}

const COMMAND: u8 = 0x15;
const REPLY_HEADER: usize = 8;
const OK: u8 = 0x01;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Step {
    Address,
    Key,
    Challenge,
    Commit,
}

impl Step {
    fn subcommand(self) -> u8 {
        match self {
            Step::Address => 0x01,
            Step::Key => 0x04,
            Step::Challenge => 0x02,
            Step::Commit => 0x03,
        }
    }
}

/// This computer's address and two blocks drawn fresh for each pairing: fixed blocks would make
/// the key the Joy-Con derives public.
#[derive(Clone)]
pub struct Secrets {
    pub host: HostAddress,
    pub a1: [u8; 16],
    pub a2: [u8; 16],
}

impl fmt::Debug for Secrets {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Secrets").field("host", &self.host).finish_non_exhaustive()
    }
}

/// The Joy-Con proves it took the key by encrypting A2 with it:
/// AES-128(key = reverse(A1 xor B1), block = reverse(A2)).
pub fn expected_answer(a1: &[u8; 16], b1: &[u8; 16], a2: &[u8; 16]) -> [u8; 16] {
    let mut key: [u8; 16] = std::array::from_fn(|i| a1[i] ^ b1[i]);
    key.reverse();
    let mut block = *a2;
    block.reverse();
    let mut block = Array::from(block);
    Aes128::new(&Array::from(key)).encrypt_block(&mut block);
    block.into()
}

fn frame(step: Step, data: &[u8]) -> Vec<u8> {
    let mut frame =
        vec![COMMAND, 0x91, 0x01, step.subcommand(), 0x00, data.len() as u8, 0x00, 0x00];
    frame.extend_from_slice(data);
    frame
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Progress {
    /// Write this next.
    Send(Vec<u8>),
    Done,
    Failed(String),
    /// Not a reply to the step in flight.
    Ignored,
}

#[derive(Debug)]
pub struct Pairing {
    secrets: Secrets,
    step: Option<Step>,
    b1: [u8; 16],
}

impl Pairing {
    pub fn new(secrets: Secrets) -> Self {
        Self { secrets, step: Some(Step::Address), b1: [0; 16] }
    }

    /// Slot one is this address and slot two the address with its lowest byte minus one, as a
    /// Switch 2 sends.
    pub fn first(&self) -> Vec<u8> {
        let host = self.secrets.host.wire();
        let mut second = host;
        second[0] = second[0].wrapping_sub(1);
        frame(Step::Address, &[&[0x00, 0x02][..], &host, &second].concat())
    }

    pub fn receive(&mut self, reply: &[u8]) -> Progress {
        let Some(step) = self.step else { return Progress::Ignored };
        if reply.len() < 4 || reply[0] != COMMAND || reply[3] != step.subcommand() {
            return Progress::Ignored;
        }
        let progress = Self::advance(step, reply, &self.secrets, &mut self.b1);
        self.step = match &progress {
            Progress::Send(_) => match step {
                Step::Address => Some(Step::Key),
                Step::Key => Some(Step::Challenge),
                Step::Challenge => Some(Step::Commit),
                Step::Commit => None,
            },
            _ => None,
        };
        progress
    }

    fn advance(step: Step, reply: &[u8], secrets: &Secrets, b1: &mut [u8; 16]) -> Progress {
        let failed = |why: &str| Progress::Failed(format!("0x15/{:02X}: {why}", step.subcommand()));
        if reply[1] != OK || reply.get(REPLY_HEADER) != Some(&OK) {
            return failed("refused");
        }
        let data = &reply[REPLY_HEADER + 1..];
        let block = || data.get(..16).and_then(|b| <[u8; 16]>::try_from(b).ok());
        match step {
            Step::Address => Progress::Send(frame(Step::Key, &[&[0x00][..], &secrets.a1].concat())),
            Step::Key => {
                let Some(b) = block() else { return failed("reply too short") };
                *b1 = b;
                Progress::Send(frame(Step::Challenge, &[&[0x00][..], &secrets.a2].concat()))
            }
            Step::Challenge => match block() {
                None => failed("reply too short"),
                Some(b2) if b2 != expected_answer(&secrets.a1, b1, &secrets.a2) => {
                    failed("wrong answer to the challenge")
                }
                Some(_) => Progress::Send(frame(Step::Commit, &[0x00])),
            },
            Step::Commit => Progress::Done,
        }
    }
}
