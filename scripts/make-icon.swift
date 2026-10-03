// Draws the JoyMouse app icon and writes it as an .icns file.
// Usage: swift scripts/make-icon.swift <output.icns> [preview.png]
import AppKit

let canvas: CGFloat = 1024

func color(_ hex: UInt32, alpha: CGFloat = 1) -> NSColor {
    NSColor(srgbRed: CGFloat((hex >> 16) & 0xFF) / 255, green: CGFloat((hex >> 8) & 0xFF) / 255,
            blue: CGFloat(hex & 0xFF) / 255, alpha: alpha)
}

func drawIcon() {
    // Squircle-ish tile on Apple's icon grid: 824 pt content, 100 pt margin.
    let tile = NSRect(x: 100, y: 100, width: 824, height: 824)
    let tilePath = NSBezierPath(roundedRect: tile, xRadius: 185, yRadius: 185)
    NSGraphicsContext.saveGraphicsState()
    let shadow = NSShadow()
    shadow.shadowColor = color(0x000000, alpha: 0.28)
    shadow.shadowBlurRadius = 24
    shadow.shadowOffset = NSSize(width: 0, height: 10)
    shadow.set()
    NSGradient(starting: color(0xFF5A46), ending: color(0xD9271A))!.draw(in: tilePath, angle: 90)
    NSGraphicsContext.restoreGraphicsState()

    // Joy-Con body: a tall pill, slightly left of centre.
    let body = NSRect(x: 300, y: 210, width: 300, height: 600)
    color(0xFFFFFF).setFill()
    NSBezierPath(roundedRect: body, xRadius: 150, yRadius: 150).fill()

    // Stick and one face button.
    color(0x2B2B2E).setFill()
    NSBezierPath(ovalIn: NSRect(x: 380, y: 290, width: 140, height: 140)).fill()
    color(0x3A3A3E).setFill()
    NSBezierPath(ovalIn: NSRect(x: 425, y: 520, width: 50, height: 50)).fill()

    // Mouse pointer overlapping the lower right of the controller.
    let points: [NSPoint] = [(0, 0), (0, 17), (4, 13), (7, 20), (9.4, 19), (6.4, 12.2), (12, 12)]
        .map { NSPoint(x: 540 + $0.0 * 17, y: 520 + $0.1 * 17) }
    let pointer = NSBezierPath()
    pointer.move(to: points[0])
    points.dropFirst().forEach { pointer.line(to: $0) }
    pointer.close()
    pointer.lineJoinStyle = .round
    pointer.lineWidth = 22
    color(0xFFFFFF).setStroke()
    pointer.stroke()
    color(0x111114).setFill()
    pointer.fill()
}

func png(pixels: Int) -> Data {
    let rep = NSBitmapImageRep(bitmapDataPlanes: nil, pixelsWide: pixels, pixelsHigh: pixels,
                               bitsPerSample: 8, samplesPerPixel: 4, hasAlpha: true, isPlanar: false,
                               colorSpaceName: .deviceRGB, bytesPerRow: 0, bitsPerPixel: 0)!
    rep.size = NSSize(width: canvas, height: canvas)
    NSGraphicsContext.saveGraphicsState()
    let context = NSGraphicsContext(bitmapImageRep: rep)!
    NSGraphicsContext.current = context
    // Flip so the drawing code can use top-left coordinates.
    context.cgContext.translateBy(x: 0, y: canvas)
    context.cgContext.scaleBy(x: 1, y: -1)
    drawIcon()
    NSGraphicsContext.restoreGraphicsState()
    return rep.representation(using: .png, properties: [:])!
}

let arguments = CommandLine.arguments
guard arguments.count >= 2 else {
    FileHandle.standardError.write("usage: make-icon.swift <output.icns> [preview.png]\n".data(using: .utf8)!)
    exit(1)
}

let iconset = FileManager.default.temporaryDirectory
    .appendingPathComponent("AppIcon-\(ProcessInfo.processInfo.processIdentifier).iconset")
try FileManager.default.createDirectory(at: iconset, withIntermediateDirectories: true)
defer { try? FileManager.default.removeItem(at: iconset) }

for size in [16, 32, 128, 256, 512] {
    try png(pixels: size).write(to: iconset.appendingPathComponent("icon_\(size)x\(size).png"))
    try png(pixels: size * 2).write(to: iconset.appendingPathComponent("icon_\(size)x\(size)@2x.png"))
}
if arguments.count >= 3 {
    try png(pixels: 1024).write(to: URL(fileURLWithPath: arguments[2]))
}

let iconutil = Process()
iconutil.executableURL = URL(fileURLWithPath: "/usr/bin/iconutil")
iconutil.arguments = ["-c", "icns", iconset.path, "-o", arguments[1]]
try iconutil.run()
iconutil.waitUntilExit()
exit(iconutil.terminationStatus)
