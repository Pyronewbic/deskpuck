import DeskpuckFFI
import Foundation

/// What a button does, as config.json stores it: a bare key code, a
/// shortcut {"key", "modifiers"}, or a modifier button {"modifier", "latch"}.
enum KeyMapping: Codable, Hashable {
    case key(Int, modifiers: [String] = [])
    case modifier(String, latch: Bool)

    private enum CodingKeys: String, CodingKey { case key, modifiers, modifier, latch }

    init(from decoder: Decoder) throws {
        if let key = try? decoder.singleValueContainer().decode(Int.self) {
            self = .key(key)
            return
        }
        let fields = try decoder.container(keyedBy: CodingKeys.self)
        if let modifier = try fields.decodeIfPresent(String.self, forKey: .modifier) {
            self = .modifier(modifier, latch: try fields.decodeIfPresent(Bool.self, forKey: .latch) ?? false)
        } else {
            self = .key(try fields.decode(Int.self, forKey: .key),
                        modifiers: try fields.decodeIfPresent([String].self, forKey: .modifiers) ?? [])
        }
    }

    func encode(to encoder: Encoder) throws {
        switch self {
        case .key(let key, let modifiers) where modifiers.isEmpty:
            var value = encoder.singleValueContainer()
            try value.encode(key)
        case .key(let key, let modifiers):
            var fields = encoder.container(keyedBy: CodingKeys.self)
            try fields.encode(key, forKey: .key)
            try fields.encode(modifiers, forKey: .modifiers)
        case .modifier(let modifier, let latch):
            var fields = encoder.container(keyedBy: CodingKeys.self)
            try fields.encode(modifier, forKey: .modifier)
            try fields.encode(latch, forKey: .latch)
        }
    }
}

/// The settings in config.json. The Rust core validates and saves them.
struct DeskpuckConfig: Codable, Equatable {
    var version = 1
    var keyMappings: [String: KeyMapping]
    var pointerSpeed: Double
    var repeatDelay: Double
    var repeatInterval: Double
    var scrollEnabled: Bool
    /// "system", "light" or "dark", for Deskpuck's own windows.
    var appearance: String
}

struct CoreError: LocalizedError {
    let errorDescription: String?
}

/// Settings calls into the Rust core.
enum Core {
    /// A stale or mismatched static library must fail loudly, not misbehave.
    static func checkLibrary() {
        guard dp_abi_version() == UInt32(DP_ABI_VERSION) else {
            fatalError("Deskpuck's Rust library is out of date; run scripts/build-rust.sh and rebuild.")
        }
    }

    /// Takes ownership of a string returned by the library.
    static func take(_ pointer: UnsafeMutablePointer<CChar>?) -> String? {
        guard let pointer else { return nil }
        defer { dp_string_free(pointer) }
        return String(cString: pointer)
    }

    static func json(_ config: DeskpuckConfig) -> String {
        let encoder = JSONEncoder()
        encoder.outputFormatting = .sortedKeys
        return (try? encoder.encode(config)).flatMap { String(data: $0, encoding: .utf8) } ?? "{}"
    }

    private static func decode<T: Decodable>(_ type: T.Type, _ string: String?) -> T? {
        string.flatMap { $0.data(using: .utf8) }.flatMap { try? JSONDecoder().decode(type, from: $0) }
    }

    static var defaultFileURL: URL {
        if let path = take(dp_config_default_path()) {
            return URL(fileURLWithPath: path)
        }
        return FileManager.default.homeDirectoryForCurrentUser
            .appendingPathComponent("Library/Application Support/Deskpuck/config.json")
    }

    static var defaults: DeskpuckConfig {
        decode(DeskpuckConfig.self, take(dp_config_defaults()))
            ?? DeskpuckConfig(keyMappings: [:], pointerSpeed: 1, repeatDelay: 0.4, repeatInterval: 0.06, scrollEnabled: true, appearance: "system")
    }

    /// Never fails: anything unusable falls back to its default and is described in the warnings.
    static func load(from url: URL) -> (DeskpuckConfig, [String]) {
        struct Loaded: Decodable {
            let config: DeskpuckConfig
            let warnings: [String]
        }
        let loaded = url.path.withCString { decode(Loaded.self, take(dp_config_load($0))) }
        return loaded.map { ($0.config, $0.warnings) } ?? (defaults, ["Settings could not be read; using defaults."])
    }

    static func problems(_ config: DeskpuckConfig) -> [String] {
        json(config).withCString { decode([String].self, take(dp_config_problems($0))) } ?? ["Settings could not be checked."]
    }

    /// Validates, then replaces the file atomically.
    static func save(_ config: DeskpuckConfig, to url: URL) throws {
        let message = json(config).withCString { config in
            url.path.withCString { path in take(dp_config_save(config, path)) }
        }
        if let message {
            throw CoreError(errorDescription: message)
        }
    }
}

enum ConnectionState {
    case bluetoothOff, bluetoothUnauthorized, unavailable, notPaired, pairing, searching, inUseElsewhere, connecting, connected

    init(_ status: dp_status) {
        switch status {
        case DP_STATUS_BLUETOOTH_UNAUTHORIZED: self = .bluetoothUnauthorized
        case DP_STATUS_UNAVAILABLE: self = .unavailable
        case DP_STATUS_NOT_PAIRED: self = .notPaired
        case DP_STATUS_PAIRING: self = .pairing
        case DP_STATUS_SEARCHING: self = .searching
        case DP_STATUS_IN_USE_ELSEWHERE: self = .inUseElsewhere
        case DP_STATUS_CONNECTING: self = .connecting
        case DP_STATUS_CONNECTED: self = .connected
        default: self = .bluetoothOff
        }
    }
}

/// The Joy-Con connection, run by the Rust core on its own thread.
@MainActor
final class Controller {
    private(set) var connectionState = ConnectionState.bluetoothOff
    private(set) var deviceName: String?
    /// Config names of the modifiers latched on by modifier buttons.
    private(set) var latchedModifiers: [String] = []
    /// When the open pairing window closes; nil when none is open.
    private(set) var pairingEndsAt: Date?
    /// From Pair New Joy-Con until a Joy-Con connects or the window closes,
    /// including while one it accepted is still connecting, which Cancel drops.
    private(set) var isPairing = false
    var stateDidChange: (() -> Void)?

    // Retained for the library's callbacks; it holds the controller weakly, so a
    // callback queued during shutdown finds nothing rather than a freed object.
    private final class Relay {
        weak var target: Controller?
    }

    private var config: DeskpuckConfig
    private var handle: OpaquePointer?
    private var relay: Unmanaged<Relay>?
    private var paused = false

    init(config: DeskpuckConfig) {
        self.config = config
    }

    func start() {
        guard handle == nil else { return }
        let relay = Relay()
        relay.target = self
        let context = Unmanaged.passRetained(relay)
        let callback: dp_status_callback = { context, status, name in
            guard let context else { return }
            let relay = Unmanaged<Relay>.fromOpaque(context).takeUnretainedValue()
            let name = name.map { String(cString: $0) }
            DispatchQueue.main.async {
                relay.target?.update(ConnectionState(status), name)
            }
        }
        let onLatch: dp_latch_callback = { context, bits in
            guard let context else { return }
            let relay = Unmanaged<Relay>.fromOpaque(context).takeUnretainedValue()
            let names = KeyChoice.modifierNames.filter { bits & $0.bit != 0 }.map(\.name)
            DispatchQueue.main.async {
                relay.target?.latched(names)
            }
        }
        handle = Core.json(config).withCString { dp_controller_start($0, callback, onLatch, context.toOpaque()) }
        if handle == nil {
            context.release()
            update(.unavailable, nil)
        } else {
            self.relay = context
            dp_controller_set_paused(handle, paused)
        }
    }

    /// Releases held input and disconnects; no callback arrives after this.
    func stop() {
        dp_controller_free(handle)
        handle = nil
        relay?.release()
        relay = nil
    }

    var isPaused: Bool {
        get { paused }
        set {
            paused = newValue
            dp_controller_set_paused(handle, newValue)
        }
    }

    /// The first Joy-Con that connects in the next DP_PAIRING_SECONDS replaces the paired one.
    func startPairing() {
        guard handle != nil, !paused else { return }
        pairingEndsAt = Date().addingTimeInterval(TimeInterval(DP_PAIRING_SECONDS))
        isPairing = true
        dp_controller_start_pairing(handle)
    }

    func cancelPairing() {
        isPairing = false
        dp_controller_cancel_pairing(handle)
        stateDidChange?()
    }

    /// Takes effect immediately. Throws the problems if the settings are invalid.
    func apply(_ config: DeskpuckConfig) throws {
        let problems = Core.problems(config)
        guard problems.isEmpty else {
            throw CoreError(errorDescription: problems.joined(separator: " "))
        }
        self.config = config
        // Not started yet: start() picks the new config up.
        guard let handle else { return }
        if let message = Core.json(config).withCString({ Core.take(dp_controller_apply_config(handle, $0)) }) {
            throw CoreError(errorDescription: message)
        }
    }

    private func latched(_ names: [String]) {
        latchedModifiers = names
        stateDidChange?()
    }

    private func update(_ state: ConnectionState, _ name: String?) {
        connectionState = state
        deviceName = name
        if state != .pairing {
            pairingEndsAt = nil
        }
        if state != .pairing && state != .connecting {
            isPairing = false
        }
        stateDidChange?()
    }
}
