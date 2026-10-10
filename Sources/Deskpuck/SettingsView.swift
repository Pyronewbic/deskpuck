import SwiftUI

enum SettingsTab: String, CaseIterable {
    case buttons = "Buttons"
    case general = "General"

    var width: CGFloat {
        switch self {
        case .buttons: 600
        case .general: 440
        }
    }
}

struct SettingsView: View {
    @ObservedObject var model: SettingsModel
    @State private var tab = SettingsTab.buttons
    @State private var confirmingRestore = false
    @State private var contentHeight: CGFloat = 0
    @State private var chromeHeight: CGFloat = 0
    @State private var screenHeight = Self.visibleScreenHeight()

    var body: some View {
        VStack(spacing: 0) {
            Picker("Section", selection: tabBinding) {
                ForEach(SettingsTab.allCases, id: \.self) { tab in
                    Text(tab.rawValue).tag(tab)
                }
            }
            .pickerStyle(.segmented)
            .labelsHidden()
            .fixedSize()
            .padding(.top, 16)
            .measured(ChromeHeight.self)
            // Only the content scrolls; the window is as tall as it, up to the cap.
            ScrollView {
                content
                    .fixedSize(horizontal: false, vertical: true)
                    .measured(ContentHeight.self)
            }
            .frame(height: max(0, Core.windowHeight(content: contentHeight, chrome: chromeHeight,
                                                    screen: screenHeight) - chromeHeight))
            VStack(spacing: 0) {
                Divider()
                footer
            }
            .measured(ChromeHeight.self)
        }
        .frame(width: tab.width)
        .onPreferenceChange(ContentHeight.self) { contentHeight = $0 }
        .onPreferenceChange(ChromeHeight.self) { chromeHeight = $0 }
        .onReceive(NotificationCenter.default.publisher(for: NSWindow.didChangeScreenNotification)) { _ in
            screenHeight = Self.visibleScreenHeight()
        }
        .onReceive(NotificationCenter.default.publisher(for: NSApplication.didChangeScreenParametersNotification)) { _ in
            screenHeight = Self.visibleScreenHeight()
        }
    }

    private static func visibleScreenHeight() -> CGFloat {
        NSScreen.main?.visibleFrame.height ?? 0
    }

    private var content: some View {
        Form {
            if !model.loadWarnings.isEmpty {
                Section {
                    ForEach(model.loadWarnings, id: \.self) { warning in
                        Label(warning, systemImage: "exclamationmark.triangle")
                    }
                } header: {
                    Text("Your settings file had problems; defaults were used for these")
                }
            }

            switch tab {
            case .buttons: buttons
            case .general: general
            }
        }
        .formStyle(.grouped)
    }

    private var footer: some View {
        HStack {
            Button("Restore All Defaults...") { confirmingRestore = true }
            Spacer()
            if let error = model.saveError {
                Label(error, systemImage: "exclamationmark.triangle")
                    .foregroundStyle(.red)
                    .lineLimit(1)
                    .truncationMode(.middle)
                    .help(error)
            } else {
                Text(Self.versionText)
                    .foregroundStyle(.secondary)
            }
        }
        .padding(.horizontal, 20)
        .padding(.vertical, 12)
        .confirmationDialog("Restore all settings to their defaults?", isPresented: $confirmingRestore) {
            Button("Restore", role: .destructive) { model.restoreDefaults() }
            Button("Cancel", role: .cancel) {}
        } message: {
            Text("Your button mappings will be lost.")
        }
    }

    private static let versionText =
        "Deskpuck " + (Bundle.main.infoDictionary?["CFBundleShortVersionString"] as? String ?? "?")

    // Leaving a tab ends a recording on it, which would otherwise keep
    // taking key presses from a row no longer on screen.
    private var tabBinding: Binding<SettingsTab> {
        Binding(
            get: { tab },
            set: { newTab in
                if newTab != tab {
                    tab = newTab
                    model.recordingButton = nil
                }
            }
        )
    }

    @ViewBuilder private var buttons: some View {
        let rows = ButtonRow.rightJoyCon
        let half = rows.count / 2
        Section {
            ForEach(0..<half, id: \.self) { i in
                HStack(spacing: 28) {
                    buttonCell(rows[i])
                    buttonCell(rows[i + half])
                }
            }
        } footer: {
            Text("R is Left click and ZR is Right click. These two cannot be changed.")
        }
    }

    private func buttonCell(_ row: ButtonRow) -> some View {
        HStack {
            Text(row.label)
            Spacer(minLength: 8)
            if model.recordingButton == row.id {
                ShortcutRecorder(
                    onRecord: { mapping in
                        model.setMapping(mapping, for: row.id)
                        model.recordingButton = nil
                    },
                    onCancel: { model.recordingButton = nil }
                )
            } else {
                Picker(row.label, selection: binding(for: row.id)) {
                    ForEach(choices(for: row.id), id: \.self) { choice in
                        Text(choice.name).tag(choice)
                    }
                }
                .labelsHidden()
            }
        }
        .frame(maxWidth: .infinity)
    }

    @ViewBuilder private var general: some View {
        Section {
            LabeledContent("Pointer speed") {
                HStack {
                    Slider(value: $model.pointerSpeed, in: 0.25...4, step: 0.25)
                    Text(Self.speedText(model.pointerSpeed))
                        .monospacedDigit()
                        .frame(width: 40, alignment: .trailing)
                }
            }
            Toggle("Scroll with the stick", isOn: $model.scrollEnabled)
        } header: {
            Text("Mouse")
        }

        Section {
            Toggle("Repeat keys while held", isOn: $model.repeatEnabled)
            if model.repeatEnabled {
                LabeledContent("Delay before repeating") {
                    HStack {
                        Slider(value: $model.repeatDelay, in: 0.15...1.0, step: 0.05)
                        Text(String(format: "%.2f s", model.repeatDelay))
                            .monospacedDigit()
                            .frame(width: 52, alignment: .trailing)
                    }
                }
                LabeledContent("Repeat speed") {
                    HStack {
                        Slider(value: $model.repeatRate, in: 4...30, step: 1)
                        Text("\(Int(model.repeatRate.rounded()))/s")
                            .monospacedDigit()
                            .frame(width: 52, alignment: .trailing)
                    }
                }
            }
        } header: {
            Text("Key repeat")
        }

        Section {
            Picker("Theme", selection: $model.appearance) {
                Text("System").tag("system")
                Text("Light").tag("light")
                Text("Dark").tag("dark")
            }
            .pickerStyle(.segmented)
        } header: {
            Text("Appearance")
        }

        Section {
            Toggle("Check for updates daily", isOn: $model.checkUpdates)
        } header: {
            Text("Updates")
        } footer: {
            Text("Asks github.com for the latest version. No other data is sent.")
        }
    }

    // Trimmed decimals so 1.75 shows as "1.75x", not "1.8x"; the
    // Linux/Windows window spells it the same way.
    static func speedText(_ speed: Double) -> String {
        var text = String(format: "%.2f", speed)
        while text.hasSuffix("0") { text.removeLast() }
        if text.hasSuffix(".") { text.removeLast() }
        return text + "x"
    }

    private func binding(for button: String) -> Binding<KeyChoice> {
        Binding(
            get: { KeyChoice(mapping: model.mapping(for: button)) },
            set: { choice in
                if choice == .record {
                    model.recordingButton = button
                } else {
                    model.setMapping(choice.mapping, for: button)
                }
            }
        )
    }

    private func choices(for button: String) -> [KeyChoice] {
        let current = KeyChoice(mapping: model.mapping(for: button))
        let extra = KeyChoice.all.contains(current) ? [] : [current]
        return KeyChoice.all + extra + [.record]
    }
}

private struct ContentHeight: PreferenceKey {
    static var defaultValue: CGFloat = 0
    static func reduce(value: inout CGFloat, nextValue: () -> CGFloat) { value += nextValue() }
}

private struct ChromeHeight: PreferenceKey {
    static var defaultValue: CGFloat = 0
    static func reduce(value: inout CGFloat, nextValue: () -> CGFloat) { value += nextValue() }
}

private extension View {
    func measured<Key: PreferenceKey>(_ key: Key.Type) -> some View where Key.Value == CGFloat {
        background(GeometryReader { proxy in
            Color.clear.preference(key: key, value: proxy.size.height)
        })
    }
}
