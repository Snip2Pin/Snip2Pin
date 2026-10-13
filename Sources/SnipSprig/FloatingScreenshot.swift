import AppKit
import SnipSprigKit

@MainActor
final class FloatingScreenshot: NSWindowController, NSWindowDelegate {
    var onClose: (() -> Void)?

    init(image: NSImage, cursor: NSPoint = NSEvent.mouseLocation) {
        let screen = NSScreen.screens.first { $0.frame.contains(cursor) } ?? NSScreen.main
        let visible = screen?.visibleFrame ?? NSRect(x: 0, y: 0, width: 1280, height: 800)
        let size = ScreenshotLayout.fittedSize(image.size, on: visible)
        let frame = ScreenshotLayout.frame(size: size, near: cursor, on: visible)
        let panel = NSPanel(contentRect: frame, styleMask: [.borderless, .nonactivatingPanel], backing: .buffered, defer: false)
        panel.title = "小贴图"
        panel.level = .floating
        panel.collectionBehavior = [.canJoinAllSpaces, .fullScreenAuxiliary]
        panel.hidesOnDeactivate = false
        panel.isReleasedWhenClosed = false
        panel.isMovableByWindowBackground = true
        panel.isOpaque = false
        panel.backgroundColor = .clear
        panel.hasShadow = true
        panel.animationBehavior = .none
        super.init(window: panel)
        panel.delegate = self
        panel.contentView = ScreenshotView(image: image, size: frame.size, close: { [weak self] in self?.close() })
    }

    required init?(coder: NSCoder) { nil }

    func present() { window?.orderFrontRegardless() }

    func windowWillClose(_ notification: Notification) { onClose?() }
}

@MainActor
private final class ScreenshotView: NSView {
    private let image: NSImage
    private let closeAction: () -> Void
    private let closeButton = NSButton()
    init(image: NSImage, size: NSSize, close: @escaping () -> Void) {
        self.image = image
        self.closeAction = close
        super.init(frame: NSRect(origin: .zero, size: size))
        wantsLayer = true
        layer?.cornerRadius = 6
        layer?.masksToBounds = true
        setAccessibilityElement(true)
        setAccessibilityRole(.group)
        setAccessibilityLabel("置顶截图，拖动移动，双击关闭")

        closeButton.image = NSImage(systemSymbolName: "xmark", accessibilityDescription: "关闭贴图")
        closeButton.bezelStyle = .regularSquare
        closeButton.isBordered = false
        closeButton.contentTintColor = .white
        closeButton.wantsLayer = true
        closeButton.layer?.backgroundColor = NSColor.black.withAlphaComponent(0.65).cgColor
        closeButton.layer?.cornerRadius = 12
        closeButton.target = self
        closeButton.action = #selector(closeClicked)
        closeButton.setAccessibilityLabel("关闭贴图")
        closeButton.toolTip = "关闭贴图（也可双击图片）"
        closeButton.frame = NSRect(x: max(0, size.width - 30), y: max(0, size.height - 30), width: 24, height: 24)
        closeButton.autoresizingMask = [.minXMargin, .minYMargin]
        addSubview(closeButton)
    }

    required init?(coder: NSCoder) { nil }
    override var mouseDownCanMoveWindow: Bool { true }
    override func acceptsFirstMouse(for event: NSEvent?) -> Bool { true }

    override func draw(_ dirtyRect: NSRect) {
        image.draw(in: bounds, from: .zero, operation: .sourceOver, fraction: 1,
                   respectFlipped: true, hints: [.interpolation: NSImageInterpolation.high])
    }

    override func mouseDown(with event: NSEvent) {
        if event.clickCount == 2 { closeAction() }
        else { window?.performDrag(with: event) }
    }

    @objc private func closeClicked() { closeAction() }
}
