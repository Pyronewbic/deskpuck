import AppKit
import ApplicationServices
import DeskpuckCore
import SwiftUI

@MainActor
final class AppDelegate: NSObject, NSApplicationDelegate, NSMenuDelegate {
    private let controller: DPController
    private let settings: SettingsModel
    private var statusItem: NSStatusItem!
    private var settingsWindow: NSWindow?
    private var accessibilityTimer: Timer?

    override init() {
        let url = DPConfig.defaultFileURL()
        var warnings: NSArray?
        let config = DPConfig(contentsOf: url, warnings: &warnings)
        controller = DPController(config: config)
        settings = SettingsModel(controller: controller, fileURL: url, config: config,
                                 warnings: (warnings as? [String]) ?? [])
        super.init()
    }

    func applicationDidFinishLaunching(_ notification: Notification) {
        statusItem = NSStatusBar.system.statusItem(withLength: NSStatusItem.squareLength)
        let menu = NSMenu()
        menu.delegate = self
        statusItem.menu = menu

        controller.stateDidChange = { [weak self] in self?.updateIcon() }
        updateIcon()

        if !AXIsProcessTrusted() {
            // Shows the system prompt that leads to Privacy & Security > Accessibility.
            let options = [kAXTrustedCheckOptionPrompt.takeUnretainedValue() as String: true] as CFDictionary
            AXIsProcessTrustedWithOptions(options)
            watchAccessibility()
        }
        controller.start()
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

        let status = NSMenuItem(title: statusText(), action: nil, keyEquivalent: "")
        status.isEnabled = false
        menu.addItem(status)

        let pause = item("Pause Mouse Control", action: #selector(togglePause))
        pause.state = controller.isPaused ? .on : .off
        menu.addItem(pause)

        menu.addItem(.separator())
        menu.addItem(item("Settings...", action: #selector(showSettings), key: ","))
        menu.addItem(.separator())
        menu.addItem(item("Quit Deskpuck", action: #selector(NSApplication.terminate(_:)), key: "q", target: NSApp))
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

    private func statusText() -> String {
        let name = controller.deviceName ?? "Joy-Con"
        switch controller.connectionState {
        case .bluetoothOff: return "Bluetooth is off"
        case .bluetoothUnauthorized: return "Bluetooth access needed"
        case .searching: return "Searching: hold SYNC on the Joy-Con"
        case .connecting: return "Connecting to \(name)..."
        case .connected: return controller.isPaused ? "Connected to \(name) (paused)" : "Connected to \(name)"
        @unknown default: return "Unknown state"
        }
    }

    private func updateIcon() {
        let connected = controller.connectionState == .connected
        if controller.isPaused {
            let paused = NSImage(systemSymbolName: "pause.circle", accessibilityDescription: "Deskpuck, paused")
            paused?.isTemplate = true
            statusItem.button?.image = paused
        } else {
            statusItem.button?.image = StatusGlyph.image()
        }
        // Dimmed until a Joy-Con is connected.
        statusItem.button?.appearsDisabled = !connected
    }

    // MARK: Actions

    @objc private func togglePause() {
        controller.isPaused.toggle()
        updateIcon()
    }

    @objc private func showSettings() {
        if settingsWindow == nil {
            let window = NSWindow(contentViewController: NSHostingController(rootView: SettingsView(model: settings)))
            window.title = "Deskpuck Settings"
            window.styleMask = [.titled, .closable]
            window.isReleasedWhenClosed = false
            window.center()
            settingsWindow = window
        }
        // A menu-bar-only app must activate itself or the window opens behind others.
        NSApp.activate(ignoringOtherApps: true)
        settingsWindow?.makeKeyAndOrderFront(nil)
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
