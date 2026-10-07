//! The tray icon and its menu on Linux (StatusNotifierItem over D-Bus) and
//! Windows. All state lives in `Model`; this file wires it to the controller
//! and the tray library, and owns every tray object on the main thread.

use crate::hook::release_hook;
use crate::icon::{self, Look};
use crate::model::Model;
use deskpuck_ble::controller::{Controller, Hooks, LinkStatus, PairingSetup};
use deskpuck_core::config::Config;
use deskpuck_core::mapping::Modifiers;
use deskpuck_core::pairing::PairedDevice;
use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::rc::Rc;
use std::sync::mpsc::{self, Receiver, Sender};
use std::time::{Duration, Instant};
use tray_icon::menu::{CheckMenuItem, Menu, MenuEvent, MenuId, MenuItem, PredefinedMenuItem};
use tray_icon::{Icon, TrayIcon, TrayIconBuilder};

enum Event {
    Status(LinkStatus, Option<String>),
    Latched(Modifiers),
    Note(String),
    Menu(MenuId),
    /// Whether a tray host is showing the icon.
    Host(bool),
    /// A quit signal, or a close request on Windows.
    Quit,
}

/// Sends to the main thread and wakes it, from any thread.
#[derive(Clone)]
struct Wake {
    tx: Sender<Event>,
    #[cfg(windows)]
    thread: u32,
}

impl Wake {
    fn send(&self, event: Event) {
        let _ = self.tx.send(event);
        #[cfg(windows)]
        platform::wake(self.thread);
    }
}

struct Items {
    status: MenuItem,
    latched: MenuItem,
    note: MenuItem,
    pair: MenuItem,
    pause: CheckMenuItem,
    open_settings: MenuItem,
    reload: MenuItem,
    quit: MenuItem,
}

/// The menu as built: which optional lines are in it right now.
struct Shown {
    latched: bool,
    note: bool,
    look: Option<Look>,
}

pub fn run() -> ExitCode {
    let (tx, rx) = mpsc::channel();
    let wake = Wake {
        tx,
        #[cfg(windows)]
        thread: platform::current_thread(),
    };
    let start = Instant::now();
    let mut model = Model::default();

    let config_path = Config::default_path();
    let (menu, items) = build_menu();
    let tray = match TrayIconBuilder::new()
        .with_title("Deskpuck")
        .with_menu(Box::new(menu.clone()))
        .build()
    {
        Ok(tray) => tray,
        Err(e) => {
            eprintln!("deskpuck-tray: could not create the tray icon: {e}");
            return ExitCode::from(1);
        }
    };
    let mut shown = Shown { latched: false, note: false, look: None };
    {
        let wake = wake.clone();
        MenuEvent::set_event_handler(Some(move |event: MenuEvent| {
            wake.send(Event::Menu(event.id))
        }));
    }
    platform::quit_on_signals(wake.clone());

    // Started when a tray host first shows the icon, so it never runs unseen.
    let controller: Rc<RefCell<Option<Controller>>> = Rc::default();
    let mut started = false;
    let mut told_waiting = false;
    platform::on_session_end(release_hook(&controller), wake.clone());
    platform::watch_host(wake.clone());

    'run: loop {
        // Before waiting, so the first state shows even if no event ever comes.
        render(&model, start.elapsed().as_secs_f64(), &menu, &items, &tray, &mut shown);
        let mut next = platform::wait(&rx, model.needs_tick().then(|| Duration::from_secs(1)));
        while let Some(event) = next.take().or_else(|| rx.try_recv().ok()) {
            let now = start.elapsed().as_secs_f64();
            match event {
                Event::Status(status, name) => model.status(status, name),
                Event::Latched(modifiers) => model.latched(modifiers),
                Event::Note(message) => model.note(message),
                Event::Quit => break 'run,
                Event::Host(present) => {
                    model.set_host(present);
                    if present && !started {
                        started = true;
                        match connect(config_path.as_deref(), &mut model, &wake) {
                            Ok(running) => *controller.borrow_mut() = Some(running),
                            Err(problem) => {
                                model.fail(problem);
                                items.reload.set_enabled(false);
                                items.pause.set_enabled(false);
                            }
                        }
                    } else if let Some(controller) = controller.borrow().as_ref() {
                        controller.set_paused(model.paused() || !present);
                    }
                    if !present && !started && !told_waiting {
                        told_waiting = true;
                        eprintln!(
                            "deskpuck-tray: no system tray is showing icons (on GNOME, turn on \
                             the AppIndicator extension); not connecting until one appears"
                        );
                    }
                }
                Event::Menu(id) => {
                    if id == *items.quit.id() {
                        break 'run;
                    }
                    let controller = controller.borrow();
                    let Some(controller) = controller.as_ref() else { continue };
                    if id == *items.pair.id() {
                        if model.pairing() {
                            model.cancel_pairing();
                            controller.cancel_pairing();
                        } else if model.start_pairing(now) {
                            controller.start_pairing();
                        }
                    } else if id == *items.pause.id() {
                        let paused = !model.paused();
                        model.set_paused(paused);
                        controller.set_paused(paused || !model.host());
                    } else if id == *items.open_settings.id() {
                        if let Err(problem) = open_settings(config_path.as_deref(), &wake) {
                            model.note(problem);
                        }
                    } else if id == *items.reload.id()
                        && let Some(path) = &config_path
                    {
                        let (config, warnings) = Config::load(path);
                        if let Some(warning) = summarize(&warnings) {
                            model.note(warning);
                        }
                        controller.apply_settings(config.engine_settings());
                    }
                }
            }
        }
    }

    // Releases held input and disconnects before the icon goes away.
    controller.borrow_mut().take();
    drop(tray);
    ExitCode::SUCCESS
}

/// Loads the settings as they are now and starts the connection.
fn connect(
    config_path: Option<&Path>,
    model: &mut Model,
    wake: &Wake,
) -> Result<Controller, String> {
    let settings = match config_path {
        Some(path) => {
            let (config, warnings) = Config::load(path);
            if let Some(warning) = summarize(&warnings) {
                model.note(warning);
            }
            config.engine_settings()
        }
        None => Default::default(),
    };
    // Checked before Bluetooth: without it every event would be dropped silently.
    match deskpuck_inject::platform_sink() {
        Ok(sink) => start_controller(sink, settings, wake),
        Err(e) => Err(format!("Cannot post input: {e}")),
    }
}

fn start_controller<S: deskpuck_inject::Sink + Send + 'static>(
    sink: S,
    settings: deskpuck_core::engine::EngineSettings,
    wake: &Wake,
) -> Result<Controller, String> {
    let (status, latched, error) = (wake.clone(), wake.clone(), wake.clone());
    let hooks = Hooks {
        status: Box::new(move |s, name| status.send(Event::Status(s, name.map(str::to_owned)))),
        report: None,
        log: None,
        error: Box::new(move |message| error.send(Event::Note(message.to_owned()))),
        latched: Some(Box::new(move |m| latched.send(Event::Latched(m)))),
    };
    let pairing = PairingSetup { file: PairedDevice::default_path(), pair_at_start: false };
    Controller::start(settings, sink, hooks, pairing).map_err(|e| format!("Could not start: {e}"))
}

fn build_menu() -> (Menu, Items) {
    let disabled = |text: &str| MenuItem::new(text, false, None);
    let items = Items {
        status: disabled(""),
        latched: disabled(""),
        note: disabled(""),
        pair: MenuItem::new(crate::model::PAIR, true, None),
        pause: CheckMenuItem::new("Pause Mouse Control", true, false, None),
        open_settings: MenuItem::new("Open Settings File", true, None),
        reload: MenuItem::new("Reload Settings", true, None),
        quit: MenuItem::new("Quit Deskpuck", true, None),
    };
    let version = disabled(&format!("Deskpuck {}", env!("CARGO_PKG_VERSION")));
    let menu = Menu::new();
    let _ = menu.append_items(&[
        &items.status,
        &items.pair,
        &items.pause,
        &PredefinedMenuItem::separator(),
        &items.open_settings,
        &items.reload,
        &PredefinedMenuItem::separator(),
        &version,
        &items.quit,
    ]);
    (menu, items)
}

fn render(model: &Model, now: f64, menu: &Menu, items: &Items, tray: &TrayIcon, shown: &mut Shown) {
    items.status.set_text(model.status_text(now));
    items.pair.set_text(model.pair_label());
    items.pair.set_enabled(model.pair_enabled());
    items.pause.set_checked(model.paused());

    // Optional lines sit under the status line, latched first.
    let latched = model.latched_text();
    show_line(menu, &items.latched, latched.as_deref(), 1, &mut shown.latched);
    let note_at = 1 + usize::from(shown.latched);
    show_line(menu, &items.note, model.note_text(), note_at, &mut shown.note);

    let look =
        Look { connected: model.connected(), paused: model.paused(), latched: latched.is_some() };
    if shown.look != Some(look) {
        if let Ok(image) = Icon::from_rgba(icon::rgba(look), icon::SIZE, icon::SIZE) {
            let _ = tray.set_icon(Some(image));
        }
        shown.look = Some(look);
    }
    let tooltip = model.tooltip(now);
    #[cfg(target_os = "linux")]
    let tooltip = crate::model::escape_markup(&tooltip);
    let _ = tray.set_tooltip(Some(tooltip));
}

fn show_line(menu: &Menu, item: &MenuItem, text: Option<&str>, at: usize, shown: &mut bool) {
    match text {
        Some(text) => {
            item.set_text(text);
            if !*shown {
                *shown = menu.insert(item, at).is_ok();
            }
        }
        None if *shown => {
            *shown = menu.remove(item).is_err();
        }
        None => {}
    }
}

/// The first warning, with a count of the rest.
fn summarize(warnings: &[String]) -> Option<String> {
    let first = warnings.first()?;
    Some(match warnings.len() {
        1 => format!("Settings: {first}"),
        n => format!("Settings: {first} (and {} more)", n - 1),
    })
}

/// Creates config.json with the defaults first, so there is something to edit.
fn open_settings(path: Option<&Path>, wake: &Wake) -> Result<(), String> {
    let path = path.ok_or("No settings folder on this system")?;
    if !path.exists() {
        Config::default()
            .save(path)
            .map_err(|e| format!("Could not create {}: {e}", path.display()))?;
    }
    let mut command = platform::editor(path);
    let child = command.spawn().map_err(|e| format!("Could not open {}: {e}", path.display()))?;
    let wake = wake.clone();
    let path: PathBuf = path.to_owned();
    // Waited on a thread so the editor neither blocks the menu nor lingers as a zombie.
    std::thread::spawn(move || {
        let mut child = child;
        if !child.wait().is_ok_and(|status| status.success()) {
            wake.send(Event::Note(format!("Could not open {}", path.display())));
        }
    });
    Ok(())
}

#[cfg(target_os = "linux")]
mod platform {
    use super::{Event, Receiver, Wake};
    use std::path::Path;
    use std::process::Command;
    use std::time::Duration;

    /// The ksni backend serves D-Bus on its own thread, so no GUI loop is needed.
    pub fn wait(rx: &Receiver<Event>, timeout: Option<Duration>) -> Option<Event> {
        match timeout {
            Some(timeout) => rx.recv_timeout(timeout).ok(),
            None => rx.recv().ok(),
        }
    }

    /// Logout sends SIGTERM, handled by quit_on_signals.
    pub fn on_session_end(_release: Box<dyn FnOnce()>, _wake: Wake) {}

    const HOST_POLL: Duration = Duration::from_secs(2);

    /// Without a StatusNotifier host the icon is registered but shown nowhere
    /// (GNOME without the AppIndicator extension), and ksni still reports
    /// success, so ask the watcher directly; hosts come and go with the panel.
    pub fn watch_host(wake: Wake) {
        std::thread::spawn(move || {
            let bus = zbus::blocking::Connection::session().ok();
            let mut last = None;
            loop {
                let present = bus.as_ref().is_some_and(host_registered);
                if last != Some(present) {
                    last = Some(present);
                    wake.send(Event::Host(present));
                }
                std::thread::sleep(HOST_POLL);
            }
        });
    }

    fn host_registered(bus: &zbus::blocking::Connection) -> bool {
        let reply = bus.call_method(
            Some("org.kde.StatusNotifierWatcher"),
            "/StatusNotifierWatcher",
            Some("org.freedesktop.DBus.Properties"),
            "Get",
            &("org.kde.StatusNotifierWatcher", "IsStatusNotifierHostRegistered"),
        );
        reply
            .ok()
            .and_then(|reply| reply.body().deserialize::<zbus::zvariant::OwnedValue>().ok())
            .and_then(|value| bool::try_from(value).ok())
            .unwrap_or(false)
    }

    pub fn editor(path: &Path) -> Command {
        let mut command = Command::new("xdg-open");
        command.arg(path);
        command
    }

    /// Ctrl+C, a closed terminal and SIGTERM (logout) quit cleanly, releasing
    /// held input.
    pub fn quit_on_signals(wake: Wake) {
        std::thread::spawn(move || {
            let Ok(runtime) = tokio::runtime::Builder::new_current_thread().enable_all().build()
            else {
                return;
            };
            runtime.block_on(async {
                use tokio::signal::unix::{SignalKind, signal};
                let (Ok(mut hangup), Ok(mut terminate)) =
                    (signal(SignalKind::hangup()), signal(SignalKind::terminate()))
                else {
                    return;
                };
                tokio::select! {
                    _ = tokio::signal::ctrl_c() => {}
                    _ = hangup.recv() => {}
                    _ = terminate.recv() => {}
                }
                wake.send(Event::Quit);
            });
        });
    }
}

#[cfg(windows)]
mod platform {
    use super::{Event, Receiver, Wake};
    use std::cell::RefCell;
    use std::path::Path;
    use std::process::Command;
    use std::time::Duration;
    use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
    use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
    use windows_sys::Win32::System::Threading::{GetCurrentThreadId, INFINITE};
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        CreateWindowExW, DefWindowProcW, DispatchMessageW, MSG, MsgWaitForMultipleObjects,
        PM_REMOVE, PeekMessageW, PostThreadMessageW, QS_ALLINPUT, RegisterClassW, TranslateMessage,
        WM_CLOSE, WM_ENDSESSION, WM_NULL, WNDCLASSW, WS_OVERLAPPED,
    };

    thread_local! {
        static SESSION_END: RefCell<Option<Box<dyn FnOnce()>>> = const { RefCell::new(None) };
        static QUIT: RefCell<Option<Wake>> = const { RefCell::new(None) };
    }

    /// The taskbar shows the icon whenever Explorer runs.
    pub fn watch_host(wake: Wake) {
        wake.send(Event::Host(true));
    }

    /// Windows may end the process as soon as WM_ENDSESSION returns, so the
    /// release runs inside the handler. Only top-level windows get the
    /// message, and the tray's own hidden window ignores it. A close request
    /// (taskkill without /F) would otherwise destroy the windows and leave
    /// the connection running with no menu, so it quits instead.
    pub fn on_session_end(release: Box<dyn FnOnce()>, wake: Wake) {
        SESSION_END.with(|slot| *slot.borrow_mut() = Some(release));
        QUIT.with(|slot| *slot.borrow_mut() = Some(wake));
        let class: Vec<u16> = "DeskpuckSession\0".encode_utf16().collect();
        // SAFETY: the class name outlives both calls; the window is never shown.
        unsafe {
            let instance = GetModuleHandleW(std::ptr::null());
            let mut window_class: WNDCLASSW = std::mem::zeroed();
            window_class.lpfnWndProc = Some(session_proc);
            window_class.hInstance = instance;
            window_class.lpszClassName = class.as_ptr();
            RegisterClassW(&window_class);
            CreateWindowExW(
                0,
                class.as_ptr(),
                class.as_ptr(),
                WS_OVERLAPPED,
                0,
                0,
                0,
                0,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                instance,
                std::ptr::null(),
            );
        }
    }

    unsafe extern "system" fn session_proc(
        window: HWND,
        message: u32,
        wparam: WPARAM,
        lparam: LPARAM,
    ) -> LRESULT {
        let quit = || QUIT.with(|slot| slot.borrow().as_ref().map(|wake| wake.send(Event::Quit)));
        if message == WM_ENDSESSION && wparam != 0 {
            if let Some(release) = SESSION_END.with(|slot| slot.borrow_mut().take()) {
                release();
            }
            quit();
            return 0;
        }
        if message == WM_CLOSE {
            quit();
            return 0;
        }
        // SAFETY: forwarding this window's own message unchanged.
        unsafe { DefWindowProcW(window, message, wparam, lparam) }
    }

    pub fn current_thread() -> u32 {
        // SAFETY: no arguments; also makes sure this thread has a message queue.
        unsafe {
            let mut msg: MSG = std::mem::zeroed();
            PeekMessageW(&mut msg, std::ptr::null_mut(), 0, 0, 0);
            GetCurrentThreadId()
        }
    }

    pub fn wake(thread: u32) {
        // SAFETY: plain value arguments; a dropped wake-up only delays the event
        // until the next message (the menu's own loop drops thread messages).
        unsafe {
            PostThreadMessageW(thread, WM_NULL, 0, 0);
        }
    }

    /// The tray's hidden window lives on this thread, so its messages must be
    /// pumped here; events arrive on the channel, drained by the caller.
    pub fn wait(_rx: &Receiver<Event>, timeout: Option<Duration>) -> Option<Event> {
        let millis =
            timeout.map_or(INFINITE, |t| t.as_millis().min(u128::from(INFINITE - 1)) as u32);
        // SAFETY: a zeroed MSG is valid output space; no handles are passed.
        unsafe {
            MsgWaitForMultipleObjects(0, std::ptr::null(), 0, millis, QS_ALLINPUT);
            let mut msg: MSG = std::mem::zeroed();
            while PeekMessageW(&mut msg, std::ptr::null_mut(), 0, 0, PM_REMOVE) != 0 {
                TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
        }
        None
    }

    pub fn editor(path: &Path) -> Command {
        let mut command = Command::new("notepad.exe");
        command.arg(path);
        command
    }

    /// A windowless process gets no console signals; Quit is the clean exit.
    pub fn quit_on_signals(_wake: Wake) {}
}
