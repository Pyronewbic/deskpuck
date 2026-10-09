import AppKit
import ApplicationServices
import Combine
import ServiceManagement
import SwiftUI

@MainActor
final class AppDelegate: NSObject, NSApplicationDelegate, NSMenuDelegate, NSMenuItemValidation {
    private let controller: Controller
    private let settings: SettingsModel
    private var statusItem: NSStatusItem!
    private var settingsWindow: NSWindow?
    private var appearanceWatch: AnyCancellable?
    private var accessibilityTimer: Timer?
    private var statusLine: NSMenuItem?
    private var pairItem: NSMenuItem?
    private var menuTimer: Timer?
    private var updateTimer: Timer?
    private var availableUpdate: String?

    override init() {
        Core.checkLibrary()
        let url = Core.defaultFileURL
        let (config, warnings) = Core.load(from: url)
        controller = Controller(config: config)
        settings = SettingsModel(controller: controller, fileURL: url, config: config, warnings: warnings)
        super.init()
    }

    func applicationDidFinishLaunching(_ notification: Notification) {
        statusItem = NSStatusBar.system.statusItem(withLength: NSStatusItem.squareLength)
        let menu = NSMenu()
        menu.delegate = self
        statusItem.menu = menu

        controller.stateDidChange = { [weak self] in
            self?.updateIcon()
            self?.refreshMenu()
        }
        updateIcon()

        if !AXIsProcessTrusted() {
            // Shows the system prompt that leads to Privacy & Security > Accessibility.
            let options = [kAXTrustedCheckOptionPrompt.takeUnretainedValue() as String: true] as CFDictionary
            AXIsProcessTrustedWithOptions(options)
            watchAccessibility()
        }
        controller.start()
        if !UpdateChecker.disabledByEnvironment {
            scheduleUpdateCheck(after: .random(in: 30...120))
        }
    }

    func applicationWillTerminate(_ notification: Notification) {
        controller.stop()
    }

    // MARK: Menu

    func menuNeedsUpdate(_ menu: NSMenu) {
        menu.removeAllItems()

        if !AXIsProcessTrusted() {
            menu.addItem(item("Allow Accessibility Access...", action: #selector(openAccessibilitySettings),
                              symbol: "exclamationmark.triangle"))
            menu.addItem(.separator())
        }
        if controller.connectionState == .bluetoothUnauthorized {
            menu.addItem(item("Allow Bluetooth Access...", action: #selector(openBluetoothSettings),
                              symbol: "exclamationmark.triangle"))
            menu.addItem(.separator())
        }

        let status = NSMenuItem(title: "", action: nil, keyEquivalent: "")
        menu.addItem(status)
        statusLine = status
        let pair = item("", action: #selector(startPairing))
        menu.addItem(pair)
        pairItem = pair
        refreshMenu()

        let pause = item("Pause Mouse Control", action: #selector(togglePause))
        pause.state = controller.isPaused ? .on : .off
        menu.addItem(pause)

        menu.addItem(.separator())
        menu.addItem(item("Settings...", action: #selector(showSettings), key: ","))
        let login = item("Start at Login", action: #selector(toggleStartAtLogin))
        login.state = SMAppService.mainApp.status == .enabled ? .on : .off
        menu.addItem(login)
        menu.addItem(.separator())
        if let availableUpdate {
            menu.addItem(item("Update Available: \(availableUpdate)...", action: #selector(openReleases)))
        }
        let version = NSMenuItem(title: versionText(), action: nil, keyEquivalent: "")
        version.isEnabled = false
        menu.addItem(version)
        menu.addItem(item("Quit Deskpuck", action: #selector(NSApplication.terminate(_:)), key: "q", target: NSApp))
    }

    func menuWillOpen(_ menu: NSMenu) {
        // .common includes menu tracking, when the default run loop mode does not run.
        let timer = Timer(timeInterval: 1, repeats: true) { [weak self] _ in
            Task { @MainActor in self?.refreshMenu() }
        }
        RunLoop.main.add(timer, forMode: .common)
        menuTimer = timer
    }

    func menuDidClose(_ menu: NSMenu) {
        menuTimer?.invalidate()
        menuTimer = nil
    }

    private func refreshMenu() {
        statusLine?.title = statusText()
        let pairing = controller.isPairing
        pairItem?.title = pairing ? "Cancel Pairing" : "Pair New Joy-Con..."
        pairItem?.action = pairing ? #selector(cancelPairing) : #selector(startPairing)
    }

    // The menu enables items by validation, so isEnabled alone would be ignored.
    func validateMenuItem(_ menuItem: NSMenuItem) -> Bool {
        if menuItem.action == #selector(startPairing) {
            // Pausing stops all connecting, so a window opened now could never pair.
            return !controller.isPaused
        }
        return true
    }

    private func item(_ title: String, action: Selector, key: String = "", symbol: String? = nil,
                      target: AnyObject? = nil) -> NSMenuItem {
        let item = NSMenuItem(title: title, action: action, keyEquivalent: key)
        item.target = target ?? self
        if let symbol {
            item.image = NSImage(systemSymbolName: symbol, accessibilityDescription: nil)
        }
        return item
    }

    private func versionText() -> String {
        let info = Bundle.main.infoDictionary ?? [:]
        let version = info["CFBundleShortVersionString"] as? String ?? "?"
        let build = info["CFBundleVersion"] as? String ?? "?"
        return "Deskpuck \(version) (\(build))"
    }

    private func statusText() -> String {
        let name = controller.deviceName ?? "Joy-Con"
        switch controller.connectionState {
        case .bluetoothOff: return "Bluetooth is off"
        case .bluetoothUnauthorized: return "Bluetooth access needed"
        case .unavailable: return "Bluetooth is unavailable"
        case .notPaired: return "Not paired: choose Pair New Joy-Con"
        case .pairing:
            let left = controller.pairingEndsAt.map { max(0, Int($0.timeIntervalSinceNow.rounded())) }
            let suffix = left.map { " (\($0) s left)" } ?? ""
            return controller.isPaused ? "Pairing paused" : "Pairing: hold SYNC on the Joy-Con\(suffix)"
        case .searching:
            return controller.isPaused ? "Paused: not looking for a Joy-Con" : "Searching: hold SYNC on the paired Joy-Con"
        case .inUseElsewhere:
            return controller.isPaused ? "Paused: not looking for a Joy-Con" : "Joy-Con is in use by another app"
        case .connecting: return controller.isPaused ? "Connecting to \(name)... (paused)" : "Connecting to \(name)..."
        case .connected: return controller.isPaused ? "Connected to \(name) (paused)" : "Connected to \(name)"
        @unknown default: return "Unknown state"
        }
    }

    private func updateIcon() {
        let latched = controller.latchedModifiers.map(KeyChoice.label).joined(separator: "+")
        statusItem.length = latched.isEmpty ? NSStatusItem.squareLength : NSStatusItem.variableLength
        statusItem.button?.title = latched.isEmpty ? "" : " " + latched
        statusItem.button?.imagePosition = .imageLeading
        let connected = controller.connectionState == .connected
        if controller.isPaused {
            let paused = NSImage(systemSymbolName: "pause.circle", accessibilityDescription: "Deskpuck, paused")
            paused?.isTemplate = true
            statusItem.button?.image = paused
        } else {
            statusItem.button?.image = StatusGlyph.image()
        }
        statusItem.button?.appearsDisabled = !connected
    }

    // MARK: Updates

    private func scheduleUpdateCheck(after seconds: TimeInterval) {
        let timer = Timer(timeInterval: seconds, repeats: false) { [weak self] _ in
            Task { @MainActor in await self?.checkForUpdate() }
        }
        RunLoop.main.add(timer, forMode: .common)
        updateTimer = timer
    }

    private func checkForUpdate() async {
        scheduleUpdateCheck(after: 24 * 60 * 60 + .random(in: 0...(60 * 60)))
        // The file, not the window's state: an unreadable file leaves the choice unknown.
        guard Core.updateAllowed(by: Core.defaultFileURL),
              let current = Bundle.main.infoDictionary?["CFBundleShortVersionString"] as? String,
              let newer = await UpdateChecker.newerVersion(than: current)
        else { return }
        availableUpdate = newer
    }

    // The address is a constant; nothing from the update check reaches the browser.
    @objc private func openReleases() {
        NSWorkspace.shared.open(UpdateChecker.releasesURL)
    }

    // MARK: Actions

    @objc private func startPairing() {
        controller.startPairing()
    }

    @objc private func cancelPairing() {
        controller.cancelPairing()
    }

    @objc private func togglePause() {
        controller.isPaused.toggle()
        updateIcon()
    }

    @objc private func toggleStartAtLogin() {
        let service = SMAppService.mainApp
        do {
            if service.status == .enabled {
                try service.unregister()
            } else {
                try service.register()
            }
        } catch {
            let alert = NSAlert()
            alert.messageText = "Could not change Start at Login"
            alert.informativeText = error.localizedDescription
            alert.runModal()
        }
        // macOS can hold a new login item until it is allowed in System Settings.
        if service.status == .requiresApproval {
            SMAppService.openSystemSettingsLoginItems()
        }
    }

    @objc private func showSettings() {
        if settingsWindow == nil {
            let window = NSWindow(contentViewController: NSHostingController(rootView: SettingsView(model: settings)))
            window.title = "Deskpuck Settings"
            window.styleMask = [.titled, .closable]
            window.isReleasedWhenClosed = false
            window.center()
            settingsWindow = window
            // Only this window: NSApp.appearance would restyle the menu bar menu too.
            appearanceWatch = settings.$appearance.sink { [weak window] name in
                window?.appearance = Self.appearance(named: name)
            }
        }
        // A menu-bar-only app must activate itself or the window opens behind others.
        NSApp.activate(ignoringOtherApps: true)
        settingsWindow?.makeKeyAndOrderFront(nil)
    }

    private static func appearance(named name: String) -> NSAppearance? {
        switch name {
        case "light": NSAppearance(named: .aqua)
        case "dark": NSAppearance(named: .darkAqua)
        default: nil
        }
    }

    @objc private func openAccessibilitySettings() {
        openSettingsPane("x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility")
    }

    @objc private func openBluetoothSettings() {
        openSettingsPane("x-apple.systempreferences:com.apple.preference.security?Privacy_Bluetooth")
    }

    private func openSettingsPane(_ url: String) {
        if let url = URL(string: url) {
            NSWorkspace.shared.open(url)
        }
    }

    // macOS sends no notification when Accessibility is granted, so poll until it is.
    private func watchAccessibility() {
        accessibilityTimer = Timer.scheduledTimer(withTimeInterval: 2, repeats: true) { [weak self] timer in
            Task { @MainActor in
                if AXIsProcessTrusted() {
                    timer.invalidate()
                    self?.accessibilityTimer = nil
                }
            }
        }
    }
}
