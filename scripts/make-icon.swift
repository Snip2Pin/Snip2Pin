import AppKit

// Generate bitmap assets from native drawing, with no third-party images.
let destination = URL(fileURLWithPath: CommandLine.arguments[1])
for size in [16, 32, 128, 256, 512] {
    for scale in [1, 2] {
        let pixels = size * scale
        let representation = NSBitmapImageRep(
            bitmapDataPlanes: nil, pixelsWide: pixels, pixelsHigh: pixels,
            bitsPerSample: 8, samplesPerPixel: 4, hasAlpha: true, isPlanar: false,
            colorSpaceName: .deviceRGB, bytesPerRow: 0, bitsPerPixel: 0
        )!
        NSGraphicsContext.saveGraphicsState()
        NSGraphicsContext.current = NSGraphicsContext(bitmapImageRep: representation)
        let side = CGFloat(pixels)
        let tile = NSBezierPath(roundedRect: NSRect(x: side * 0.06, y: side * 0.06,
                                                   width: side * 0.88, height: side * 0.88),
                                xRadius: side * 0.20, yRadius: side * 0.20)
        NSGradient(starting: NSColor(calibratedRed: 0.27, green: 0.58, blue: 0.46, alpha: 1),
                   ending: NSColor(calibratedRed: 0.12, green: 0.33, blue: 0.27, alpha: 1))!
            .draw(in: tile, angle: -75)
        let card = NSBezierPath(roundedRect: NSRect(x: side * 0.22, y: side * 0.27,
                                                   width: side * 0.56, height: side * 0.42),
                                xRadius: side * 0.06, yRadius: side * 0.06)
        NSColor.white.withAlphaComponent(0.96).setFill()
        card.fill()
        if let pin = NSImage(systemSymbolName: "pin.fill", accessibilityDescription: nil) {
            let rect = NSRect(x: side * 0.43, y: side * 0.48, width: side * 0.30, height: side * 0.30)
            pin.draw(in: rect)
        }
        NSGraphicsContext.restoreGraphicsState()
        let data = representation.representation(using: .png, properties: [:])!
        let suffix = scale == 2 ? "@2x" : ""
        try data.write(to: destination.appendingPathComponent("icon_\(size)x\(size)\(suffix).png"))
    }
}
