import AppKit

enum StatusGlyph {
    // 18 pt template image echoing the app icon: a tilted controller outline and a pointer.
    // macOS tints template images to suit the menu bar, so only the alpha matters.
    static func image() -> NSImage {
        let image = NSImage(size: NSSize(width: 18, height: 18), flipped: true) { _ in
            let pill = NSBezierPath(roundedRect: NSRect(x: 2.4, y: 1.4, width: 7.2, height: 14.6), xRadius: 3.6, yRadius: 3.6)
            var tilt = AffineTransform(translationByX: 6, byY: 8.7)
            tilt.rotate(byDegrees: -10)
            tilt.translate(x: -6, y: -8.7)
            pill.transform(using: tilt)
            pill.lineWidth = 1.5
            NSColor.black.setStroke()
            pill.stroke()

            NSColor.black.setFill()
            NSBezierPath(ovalIn: NSRect(x: 3.7, y: 3.9, width: 3.4, height: 3.4)).fill()

            let pointer = NSBezierPath()
            let points: [NSPoint] = [(9.6, 7.4), (9.6, 16.2), (11.6, 14.2), (13.1, 17.4), (14.5, 16.8), (13.0, 13.6), (15.9, 13.6)]
                .map { NSPoint(x: $0.0, y: $0.1) }
            pointer.move(to: points[0])
            points.dropFirst().forEach { pointer.line(to: $0) }
            pointer.close()
            pointer.lineJoinStyle = .round

            // Cut a thin gap around the pointer so it stays readable where it overlaps the outline.
            NSGraphicsContext.current?.compositingOperation = .clear
            pointer.lineWidth = 1.6
            pointer.stroke()
            NSGraphicsContext.current?.compositingOperation = .sourceOver
            pointer.fill()
            return true
        }
        image.isTemplate = true
        image.accessibilityDescription = "Deskpuck"
        return image
    }
}
