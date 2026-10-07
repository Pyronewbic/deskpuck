//! Starts the real tray on a private D-Bus session bus and reads its menu,
//! tooltip and icon back the way a panel would. Needs dbus-daemon, no display.
#![cfg(target_os = "linux")]

use std::collections::HashMap;
use std::io::{BufRead, BufReader};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};
use zbus::blocking::Connection;
use zbus::zvariant::OwnedValue;

struct Reap(Child);

impl Drop for Reap {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

fn session_bus() -> (Reap, String) {
    let mut daemon = Command::new("dbus-daemon")
        .args(["--session", "--nofork", "--print-address=1"])
        .stdout(Stdio::piped())
        .spawn()
        .expect("dbus-daemon is needed for this test (apt install dbus)");
    let mut address = String::new();
    BufReader::new(daemon.stdout.take().unwrap()).read_line(&mut address).unwrap();
    (Reap(daemon), address.trim().to_owned())
}

fn eventually<T>(what: &str, mut probe: impl FnMut() -> Option<T>) -> T {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        if let Some(value) = probe() {
            return value;
        }
        assert!(Instant::now() < deadline, "timed out waiting for {what}");
        std::thread::sleep(Duration::from_millis(100));
    }
}

type Pixmaps = Vec<(i32, i32, Vec<u8>)>;

fn property(bus: &Connection, service: &str, name: &str) -> Option<OwnedValue> {
    let reply = bus
        .call_method(
            Some(service),
            "/StatusNotifierItem",
            Some("org.freedesktop.DBus.Properties"),
            "Get",
            &("org.kde.StatusNotifierItem", name),
        )
        .ok()?;
    reply.body().deserialize::<OwnedValue>().ok()
}

type Node = (i32, HashMap<String, OwnedValue>, Vec<OwnedValue>);

/// Stands in for a panel's StatusNotifierWatcher with a host registered.
struct Watcher;

#[zbus::interface(name = "org.kde.StatusNotifierWatcher")]
impl Watcher {
    fn register_status_notifier_item(&self, _service: &str) {}

    #[zbus(property)]
    fn is_status_notifier_host_registered(&self) -> bool {
        true
    }
}

const WAITING: &str = "Deskpuck: Waiting for a system tray";

/// The tooltip's title, and the first line of its text.
fn tooltip(bus: &Connection, service: &str) -> Option<(String, String)> {
    let value = property(bus, service, "ToolTip")?;
    let (_icon, _pixmaps, title, text) =
        <(String, Pixmaps, String, String)>::try_from(value).ok()?;
    Some((title, text.lines().next()?.to_owned()))
}

fn menu_labels(bus: &Connection, service: &str) -> Vec<String> {
    let reply = bus
        .call_method(
            Some(service),
            "/MenuBar",
            Some("com.canonical.dbusmenu"),
            "GetLayout",
            &(0i32, -1i32, Vec::<String>::new()),
        )
        .expect("GetLayout");
    let (_revision, (_id, _props, children)): (u32, Node) = reply.body().deserialize().unwrap();
    children
        .into_iter()
        .filter_map(|child| {
            let (_id, props, _children) = Node::try_from(child).ok()?;
            String::try_from(props.get("label")?.try_clone().ok()?).ok()
        })
        .collect()
}

#[test]
fn tray_publishes_its_menu_and_quits_cleanly_on_sigterm() {
    let (_daemon, address) = session_bus();
    let home = tempfile::tempdir().unwrap();
    let mut tray = Reap(
        Command::new(env!("CARGO_BIN_EXE_deskpuck"))
            .env("DBUS_SESSION_BUS_ADDRESS", &address)
            // Never reach the real Bluetooth stack or the user's settings.
            .env("DBUS_SYSTEM_BUS_ADDRESS", "unix:path=/nonexistent/deskpuck-test")
            .env("XDG_CONFIG_HOME", home.path())
            .env("HOME", home.path())
            .spawn()
            .unwrap(),
    );
    let bus =
        zbus::blocking::connection::Builder::address(address.as_str()).unwrap().build().unwrap();
    let dbus = zbus::blocking::fdo::DBusProxy::new(&bus).unwrap();
    let service = eventually("the StatusNotifierItem", || {
        dbus.list_names()
            .ok()?
            .into_iter()
            .map(|name| name.to_string())
            .find(|name| name.starts_with("org.kde.StatusNotifierItem-"))
    });

    // This bus has no tray host, so nothing may connect yet; shown before any
    // controller event, since there may never be one.
    let (title, waiting) = eventually("the tooltip", || tooltip(&bus, &service));
    assert_eq!(title, "Deskpuck", "hosts hide a tooltip without a title");
    assert_eq!(waiting, WAITING);
    let pixmaps = eventually("the icon", || {
        let pixmaps = Pixmaps::try_from(property(&bus, &service, "IconPixmap")?).ok()?;
        (!pixmaps.is_empty()).then_some(pixmaps)
    });
    assert_eq!((pixmaps[0].0, pixmaps[0].1, pixmaps[0].2.len()), (32, 32, 32 * 32 * 4));

    let labels = menu_labels(&bus, &service);
    assert_eq!(format!("Deskpuck: {}", labels[0]), WAITING, "status line first: {labels:?}");
    for expected in
        ["Pair New Joy-Con...", "Pause Mouse Control", "Open Settings File", "Reload Settings"]
    {
        assert!(labels.iter().any(|label| label == expected), "{expected} in {labels:?}");
    }
    let version = format!("Deskpuck {}", env!("CARGO_PKG_VERSION"));
    assert!(labels.contains(&version), "{version} in {labels:?}");
    assert_eq!(labels.last().map(String::as_str), Some("Quit Deskpuck"));

    let _watcher = zbus::blocking::connection::Builder::address(address.as_str())
        .unwrap()
        .name("org.kde.StatusNotifierWatcher")
        .unwrap()
        .serve_at("/StatusNotifierWatcher", Watcher)
        .unwrap()
        .build()
        .unwrap();
    let started = eventually("the tray to notice the host", || {
        let (_, line) = tooltip(&bus, &service)?;
        (line != WAITING).then_some(line)
    });
    assert!(started.starts_with("Deskpuck: "), "{started:?}");

    let pid = tray.0.id().to_string();
    assert!(Command::new("kill").args(["-TERM", &pid]).status().unwrap().success());
    let status = eventually("the tray to exit", || tray.0.try_wait().unwrap());
    assert!(status.success(), "{status:?}");
}
