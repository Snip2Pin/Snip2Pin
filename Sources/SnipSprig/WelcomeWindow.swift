import AppKit

@MainActor
final class WelcomeWindow: NSWindowController {
    private let captureAction: () -> Void

    init(hotKeyAvailable: Bool, capture: @escaping () -> Void) {
        captureAction = capture
        let window = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 400, height: 300),
                              styleMask: [.titled, .closable], backing: .buffered, defer: false)
        window.title = "SnipSprig · 小贴图"
        window.isReleasedWhenClosed = false
        window.center()
        super.init(window: window)
        window.standardWindowButton(.miniaturizeButton)?.isHidden = true
        window.standardWindowButton(.zoomButton)?.isHidden = true
        let content = NSView()
        window.contentView = content

        let icon = NSImageView(frame: NSRect(x: 174, y: 222, width: 52, height: 52))
        icon.image = NSImage(systemSymbolName: "pin.square.fill", accessibilityDescription: nil)
        icon.imageScaling = .scaleProportionallyUpOrDown
        icon.contentTintColor = NSColor(calibratedRed: 0.20, green: 0.48, blue: 0.39, alpha: 1)
        content.addSubview(icon)

        addLabel("截一张，贴在眼前。", y: 184, size: 23, weight: .semibold, to: content)
        addLabel("框选后自动置顶，拖动移动，双击关闭。", y: 151, size: 13, color: .secondaryLabelColor, to: content)
        let button = NSButton(title: "开始截图", target: self, action: #selector(captureClicked))
        button.bezelStyle = .rounded
        button.controlSize = .large
        button.keyEquivalent = "\r"
        button.frame = NSRect(x: 124, y: 94, width: 152, height: 36)
        content.addSubview(button)
        let shortcut = hotKeyAvailable ? "全局快捷键  ⌘ ⇧ 2" : "⌘⇧2 已被占用，请从菜单栏开始截图"
        addLabel(shortcut, y: 65, size: 12, color: .secondaryLabelColor, to: content)
        addLabel("关闭此窗口后，可从顶部菜单栏继续使用。", y: 25, size: 11, color: .tertiaryLabelColor, to: content)
    }

    required init?(coder: NSCoder) { nil }

    func present() {
        NSApp.activate(ignoringOtherApps: true)
        showWindow(nil)
        window?.makeKeyAndOrderFront(nil)
    }

    private func addLabel(_ text: String, y: CGFloat, size: CGFloat, weight: NSFont.Weight = .regular,
                          color: NSColor = .labelColor, to content: NSView) {
        let label = NSTextField(labelWithString: text)
        label.font = .systemFont(ofSize: size, weight: weight)
        label.textColor = color
        label.alignment = .center
        label.frame = NSRect(x: 16, y: y, width: 368, height: size + 10)
        content.addSubview(label)
    }

    @objc private func captureClicked() { captureAction() }
}
