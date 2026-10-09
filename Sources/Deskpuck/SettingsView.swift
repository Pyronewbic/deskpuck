import SwiftUI

struct SettingsView: View {
    @ObservedObject var model: SettingsModel

    var body: some View {
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

            Section {
                ForEach(ButtonRow.rightJoyCon, id: \.self) { row in
                    if model.recordingButton == row.id {
                        LabeledContent(row.label) {
                            ShortcutRecorder(
                                onRecord: { mapping in
                                    model.setMapping(mapping, for: row.id)
                                    model.recordingButton = nil
                                },
                                onCancel: { model.recordingButton = nil }
                            )
                        }
                    } else {
                        Picker(row.label, selection: binding(for: row.id)) {
                            ForEach(choices(for: row.id), id: \.self) { choice in
                                Text(choice.name).tag(choice)
                            }
                        }
                    }
                }
                LabeledContent("R", value: "Left click")
                LabeledContent("ZR", value: "Right click")
            } header: {
                Text("Buttons")
            }

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

            Section {
                HStack {
                    if let error = model.saveError {
                        Label(error, systemImage: "exclamationmark.triangle")
                            .foregroundStyle(.red)
                    }
                    Spacer()
                    Button("Restore Defaults") { model.restoreDefaults() }
                }
            }
        }
        .formStyle(.grouped)
        .frame(width: 440)
        .fixedSize(horizontal: false, vertical: true)
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
