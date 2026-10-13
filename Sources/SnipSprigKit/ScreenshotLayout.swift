import Foundation
import CoreGraphics

public enum ScreenshotLayout {
    /// Keep the aspect ratio, never enlarge, and leave room around the image.
    public static func fittedSize(_ image: CGSize, on screen: CGRect) -> CGSize {
        guard image.width > 0, image.height > 0,
              image.width.isFinite, image.height.isFinite else { return CGSize(width: 80, height: 80) }
        let availableWidth = max(1, screen.width - 40)
        let availableHeight = max(1, screen.height - 40)
        let scale = min(1, availableWidth / image.width, availableHeight / image.height)
        return CGSize(width: max(1, image.width * scale), height: max(1, image.height * scale))
    }

    /// Cocoa coordinates can be negative on secondary monitors.
    public static func frame(size: CGSize, near cursor: CGPoint, on screen: CGRect) -> CGRect {
        let width = min(max(1, size.width), screen.width)
        let height = min(max(1, size.height), screen.height)
        let x = min(max(cursor.x + 18, screen.minX), screen.maxX - width)
        let y = min(max(cursor.y - height - 18, screen.minY), screen.maxY - height)
        return CGRect(x: x, y: y, width: width, height: height)
    }
}
