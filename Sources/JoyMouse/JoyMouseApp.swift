import AppKit

@main
enum JoyMouseApp {
    @MainActor
    static func main() {
        let app = NSApplication.shared
        // NSApplication holds its delegate weakly; this local lives as long as run().
        let delegate = AppDelegate()
        app.delegate = delegate
        app.setActivationPolicy(.accessory)
        app.run()
    }
}
