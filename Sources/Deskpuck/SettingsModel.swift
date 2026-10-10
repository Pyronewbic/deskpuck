import Foundation

@MainActor
final class SettingsModel: ObservableObject {
    @Published var mappings: [String: KeyMapping] { didSet { commit() } }
    @Published var recordingButton: String?
    @Published var pointerSpeed: Double { didSet { commit() } }
    @Published var scrollEnabled: Bool { didSet { commit() } }
    @Published var repeatEnabled: Bool { didSet { commit() } }
    @Published var repeatDelay: Double { didSet { commit() } }
    // Keys per second while held; stored as repeatInterval = 1 / rate.
    @Published var repeatRate: Double { didSet { commit() } }
    @Published var appearance: String { didSet { commit() } }
    @Published var checkUpdates: Bool { didSet { commit() } }

    @Published private(set) var loadWarnings: [String]
    @Published private(set) var saveError: String?

    static let defaultRepeatRate = 1.0 / Core.defaults.repeatInterval

    private let controller: Controller
    private let fileURL: URL
    private var loading = false
    private var pendingSave: DispatchWorkItem?

    init(controller: Controller, fileURL: URL, config: DeskpuckConfig, warnings: [String]) {
        self.controller = controller
        self.fileURL = fileURL
        loadWarnings = warnings
        mappings = [:]
        pointerSpeed = 1
        scrollEnabled = true
        repeatEnabled = true
        repeatDelay = 0.4
        repeatRate = Self.defaultRepeatRate
        appearance = "system"
        checkUpdates = true
        load(config)
    }

    func restoreDefaults() {
        load(Core.defaults)
        commit()
    }

    func mapping(for button: String) -> KeyMapping? {
        mappings[button]
    }

    func setMapping(_ mapping: KeyMapping?, for button: String) {
        mappings[button] = mapping
    }

    var config: DeskpuckConfig {
        DeskpuckConfig(
            keyMappings: mappings,
            pointerSpeed: pointerSpeed,
            repeatDelay: repeatDelay,
            repeatInterval: repeatEnabled ? 1.0 / repeatRate : 0,
            scrollEnabled: scrollEnabled,
            appearance: appearance,
            checkUpdates: checkUpdates
        )
    }

    private func load(_ config: DeskpuckConfig) {
        loading = true
        mappings = config.keyMappings
        pointerSpeed = config.pointerSpeed
        scrollEnabled = config.scrollEnabled
        repeatDelay = config.repeatDelay
        repeatEnabled = config.repeatInterval > 0
        repeatRate = config.repeatInterval > 0 ? 1.0 / config.repeatInterval : Self.defaultRepeatRate
        appearance = config.appearance
        checkUpdates = config.checkUpdates
        loading = false
    }

    // Applies immediately; the file write is debounced so slider drags don't hammer the disk.
    private func commit() {
        guard !loading else { return }
        let config = self.config
        do {
            try controller.apply(config)
            saveError = nil
        } catch {
            saveError = error.localizedDescription
            return
        }
        pendingSave?.cancel()
        let url = fileURL
        let work = DispatchWorkItem { [weak self] in
            do {
                try Core.save(config, to: url)
            } catch {
                self?.saveError = "Could not save settings: \(error.localizedDescription)"
            }
        }
        pendingSave = work
        DispatchQueue.main.asyncAfter(deadline: .now() + 0.4, execute: work)
    }
}
