import DeskpuckCore
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
                    Picker(row.label, selection: binding(for: row.id)) {
                        ForEach(choices(for: row.id), id: \.self) { choice in
                            Text(choice.name).tag(choice.code)
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
                        Text(String(format: "%.2gx", model.pointerSpeed))
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

    private func binding(for button: String) -> Binding<Int> {
        Binding(
            get: { model.keyCode(for: button) },
            set: { model.setKeyCode($0, for: button) }
        )
    }

    // Keeps a hand-edited key code selectable instead of silently showing "None".
    private func choices(for button: String) -> [KeyChoice] {
        let code = model.keyCode(for: button)
        if KeyChoice.all.contains(where: { $0.code == code }) {
            return KeyChoice.all
        }
        return KeyChoice.all + [KeyChoice(code: code, name: "Key code \(code)")]
    }
}
