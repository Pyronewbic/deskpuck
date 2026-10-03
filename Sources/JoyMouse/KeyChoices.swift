import Foundation

struct KeyChoice: Hashable {
    let code: Int
    let name: String

    // Matches the "unmapped" picker entry.
    static let none = KeyChoice(code: -1, name: "None")

    static let all: [KeyChoice] = [
        .none,
        KeyChoice(code: 36, name: "Return"),
        KeyChoice(code: 53, name: "Escape"),
        KeyChoice(code: 49, name: "Space"),
        KeyChoice(code: 48, name: "Tab"),
        KeyChoice(code: 51, name: "Delete"),
        KeyChoice(code: 117, name: "Forward Delete"),
        KeyChoice(code: 126, name: "Up Arrow"),
        KeyChoice(code: 125, name: "Down Arrow"),
        KeyChoice(code: 123, name: "Left Arrow"),
        KeyChoice(code: 124, name: "Right Arrow"),
        KeyChoice(code: 116, name: "Page Up"),
        KeyChoice(code: 121, name: "Page Down"),
        KeyChoice(code: 115, name: "Home"),
        KeyChoice(code: 119, name: "End"),
    ]
}

// Right Joy-Con buttons shown in Settings, in the order they sit on the controller.
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
