import AppKit
import Carbon.HIToolbox
import DeskpuckFFI
import SwiftUI

struct KeyChoice: Hashable {
    let mapping: KeyMapping?
    let name: String
    private var isRecord = false

    static let none = KeyChoice(mapping: nil, name: "None")
    static let record: KeyChoice = {
        var choice = KeyChoice(mapping: nil, name: "Record Shortcut...")
        choice.isRecord = true
        return choice
    }()

    private init(mapping: KeyMapping?, name: String) {
        self.mapping = mapping
        self.name = name
    }

    init(mapping: KeyMapping?) {
        self.init(mapping: mapping, name: mapping.map(Self.describe) ?? "None")
    }

    private static let keys: [(Int, String)] = [
        (36, "Return"), (53, "Escape"), (49, "Space"), (48, "Tab"), (51, "Delete"),
        (117, "Forward Delete"), (126, "Up Arrow"), (125, "Down Arrow"), (123, "Left Arrow"),
        (124, "Right Arrow"), (116, "Page Up"), (121, "Page Down"), (115, "Home"), (119, "End"),
    ]

    // Built through describe() so a saved mapping matches its menu entry.
    static let all: [KeyChoice] = [.none]
        + keys.map { KeyChoice(mapping: .key($0.0)) }
        + modifierNames.map { KeyChoice(mapping: .modifier($0.name, latch: false)) }
        + modifierNames.map { KeyChoice(mapping: .modifier($0.name, latch: true)) }

    // In the order the core presses them.
    static let modifierNames: [(name: String, flag: NSEvent.ModifierFlags, label: String, bit: UInt32)] = [
        ("control", .control, "Control", UInt32(DP_MODIFIER_CONTROL)),
        ("option", .option, "Option", UInt32(DP_MODIFIER_OPTION)),
        ("shift", .shift, "Shift", UInt32(DP_MODIFIER_SHIFT)),
        ("command", .command, "Command", UInt32(DP_MODIFIER_COMMAND)),
    ]

    static func label(_ modifier: String) -> String {
        modifierNames.first { $0.name == modifier }?.label ?? modifier
    }

    static func describe(_ mapping: KeyMapping) -> String {
        switch mapping {
        case .key(let key, let modifiers):
            let labels = modifierNames.filter { modifiers.contains($0.name) }.map(\.label)
            return (labels + [keyName(key)]).joined(separator: "+")
        case .modifier(let modifier, let latch):
            return latch ? "\(label(modifier)) (tap to latch)" : "\(label(modifier)) (while held)"
        }
    }

    static func keyName(_ code: Int) -> String {
        if let known = keys.first(where: { $0.0 == code }) {
            return known.1
        }
        return character(for: code) ?? "Key code \(code)"
    }

    private static func character(for code: Int) -> String? {
        guard let source = TISCopyCurrentKeyboardLayoutInputSource()?.takeRetainedValue(),
              let property = TISGetInputSourceProperty(source, kTISPropertyUnicodeKeyLayoutData) else {
            return nil
        }
        let data = Unmanaged<CFData>.fromOpaque(property).takeUnretainedValue()
        guard let bytes = CFDataGetBytePtr(data) else { return nil }
        var deadKeys: UInt32 = 0
        var chars = [UniChar](repeating: 0, count: 4)
        var length = 0
        let status = bytes.withMemoryRebound(to: UCKeyboardLayout.self, capacity: 1) { layout in
            UCKeyTranslate(layout, UInt16(code), UInt16(kUCKeyActionDisplay), 0, UInt32(LMGetKbdType()),
                           OptionBits(kUCKeyTranslateNoDeadKeysBit), &deadKeys, chars.count, &length, &chars)
        }
        guard status == noErr, length > 0 else { return nil }
        let text = String(utf16CodeUnits: chars, count: length).trimmingCharacters(in: .whitespacesAndNewlines)
        guard !text.isEmpty, text.unicodeScalars.allSatisfy({ !CharacterSet.controlCharacters.contains($0) }) else {
            return nil
        }
        return text.uppercased()
    }
}

struct ShortcutRecorder: View {
    let onRecord: (KeyMapping) -> Void
    let onCancel: () -> Void
    @State private var monitor: Any?

    var body: some View {
        HStack {
            Text("Press a shortcut (Esc cancels)")
                .foregroundStyle(.secondary)
            Button("Cancel", action: onCancel)
        }
        .onAppear {
            // onAppear can repeat before onDisappear; a second monitor would
            // outlive the row and swallow every key press in the app.
            removeMonitor()
            monitor = NSEvent.addLocalMonitorForEvents(matching: .keyDown) { event in
                handle(event)
                return nil
            }
        }
        .onDisappear {
            removeMonitor()
        }
    }

    private func removeMonitor() {
        if let monitor {
            NSEvent.removeMonitor(monitor)
        }
        monitor = nil
    }

    private func handle(_ event: NSEvent) {
        let flags = event.modifierFlags.intersection(.deviceIndependentFlagsMask)
        let modifiers = KeyChoice.modifierNames.filter { flags.contains($0.flag) }.map(\.name)
        let code = Int(event.keyCode)
        if code == kVK_Escape && modifiers.isEmpty {
            onCancel()
        } else if code <= 127 {
            onRecord(.key(code, modifiers: modifiers))
        }
    }
}

// Mappings for other buttons (e.g. a left Joy-Con) are kept but not shown.
struct ButtonRow: Hashable {
    let id: String
    let label: String

    static let rightJoyCon: [ButtonRow] = [
        ButtonRow(id: "RS", label: "Stick click"),
        ButtonRow(id: "A", label: "A"),
        ButtonRow(id: "B", label: "B"),
        ButtonRow(id: "X", label: "X"),
        ButtonRow(id: "Y", label: "Y"),
        ButtonRow(id: "PLUS", label: "+"),
        ButtonRow(id: "HOME", label: "Home"),
        ButtonRow(id: "CHAT", label: "C"),
        ButtonRow(id: "SL", label: "SL"),
        ButtonRow(id: "SR", label: "SR"),
    ]
}
