import AppKit
import CoreGraphics
import SnipSprigKit

@MainActor
final class AppDelegate: NSObject, NSApplicationDelegate {
    private var statusItem: NSStatusItem?
    private var hotKey: GlobalHotKey?
    private var welcome: WelcomeWindow?
    private var screenshots: [UUID: FloatingScreenshot] = [:]
    private let captureRunner = CaptureRunner()
    private var capturePending = false
    private var hotKeyAvailable = false
    private var captureMenuItem: NSMenuItem?

    func applicationDidFinishLaunching(_ notification: Notification) {
        let item = NSStatusBar.system.statusItem(withLength: NSStatusItem.squareLength)
        item.button?.image = NSImage(systemSymbolName: "pin", accessibilityDescription: "SnipSprig 小贴图")
        item.button?.toolTip = "SnipSprig · 小贴图"
        let menu = NSMenu()
        let capture = menu.addItem(withTitle: "截一张", action: #selector(startCapture), keyEquivalent: "2")
        capture.keyEquivalentModifierMask = [.command, .shift]
        capture.target = self
        captureMenuItem = capture
        menu.addItem(.separator())
        menu.addItem(withTitle: "关闭所有贴图", action: #selector(closeAll), keyEquivalent: "").target = self
        menu.addItem(withTitle: "使用说明", action: #selector(showWelcome), keyEquivalent: "").target = self
        menu.addItem(.separator())
        menu.addItem(withTitle: "退出小贴图", action: #selector(quit), keyEquivalent: "q").target = self
        item.menu = menu
        statusItem = item

        let key = GlobalHotKey { [weak self] in self?.startCapture() }
        hotKeyAvailable = key.register()
        hotKey = key
        if CommandLine.arguments.contains("--demo") {
            pin(Self.demoImage())
        } else if !UserDefaults.standard.bool(forKey: "hasSeenWelcome") || !hotKeyAvailable {
            showWelcome()
            UserDefaults.standard.set(true, forKey: "hasSeenWelcome")
        }
    }

    func applicationShouldHandleReopen(_ sender: NSApplication, hasVisibleWindows flag: Bool) -> Bool {
        showWelcome()
        return false
    }

    func applicationWillTerminate(_ notification: Notification) {
        hotKey?.unregister()
        captureRunner.shutdown()
    }

    @objc func startCapture() {
        guard !capturePending else { return }
        guard CGPreflightScreenCaptureAccess() || CGRequestScreenCaptureAccess() else {
            permissionHelp()
            return
        }
        capturePending = true
        captureMenuItem?.isEnabled = false
        welcome?.window?.orderOut(nil)
        // Leave time for the menu/intro to disappear before the system picker starts.
        Task { @MainActor [weak self] in
            guard let self else { return }
            defer {
                self.capturePending = false
                self.captureMenuItem?.isEnabled = true
            }
            try? await Task.sleep(for: .milliseconds(180))
            do {
                guard let data = try await self.captureRunner.capture() else { return }
                guard let image = NSImage(data: data), image.size.width > 0, image.size.height > 0 else {
                    throw CaptureError.failed("未能读取截图，请再试一次。")
                }
                self.pin(image)
            } catch {
                self.showError(error.localizedDescription)
            }
        }
    }

    private func pin(_ image: NSImage) {
        let id = UUID()
        let screenshot = FloatingScreenshot(image: image)
        screenshot.onClose = { [weak self] in self?.screenshots.removeValue(forKey: id) }
        screenshots[id] = screenshot
        screenshot.present()
    }

    @objc private func closeAll() {
        let openScreenshots = Array(screenshots.values)
        for screenshot in openScreenshots { screenshot.close() }
    }

    @objc private func showWelcome() {
        if welcome == nil {
            welcome = WelcomeWindow(hotKeyAvailable: hotKeyAvailable) { [weak self] in self?.startCapture() }
        }
        welcome?.present()
    }

    @objc private func quit() { NSApp.terminate(nil) }

    private func permissionHelp() {
        let alert = NSAlert()
        alert.messageText = "先允许小贴图读取屏幕"
        alert.informativeText = "请在系统设置的「隐私与安全性 → 屏幕与系统音频录制」（旧版叫「屏幕录制」）里开启 SnipSprig。授权后退出并重新打开小贴图，再开始截图。"
        alert.addButton(withTitle: "打开系统设置")
        alert.addButton(withTitle: "稍后")
        NSApp.activate(ignoringOtherApps: true)
        if alert.runModal() == .alertFirstButtonReturn,
           let url = URL(string: "x-apple.systempreferences:com.apple.preference.security?Privacy_ScreenCapture") {
            NSWorkspace.shared.open(url)
        }
    }

    private func showError(_ message: String) {
        let alert = NSAlert()
        alert.messageText = "这次截图没有完成"
        alert.informativeText = message
        alert.addButton(withTitle: "好")
        NSApp.activate(ignoringOtherApps: true)
        alert.runModal()
    }

    private static func demoImage() -> NSImage {
        let image = NSImage(size: NSSize(width: 400, height: 240))
        image.lockFocus()
        NSColor(calibratedRed: 0.94, green: 0.96, blue: 0.94, alpha: 1).setFill()
        NSRect(x: 0, y: 0, width: 400, height: 240).fill()
        let title: NSString = "SnipSprig"
        title.draw(at: NSPoint(x: 28, y: 153), withAttributes: [
            .font: NSFont.systemFont(ofSize: 30, weight: .semibold),
            .foregroundColor: NSColor(calibratedRed: 0.20, green: 0.48, blue: 0.39, alpha: 1)
        ])
        ("一张截图，留在眼前。" as NSString).draw(at: NSPoint(x: 28, y: 116), withAttributes: [
            .font: NSFont.systemFont(ofSize: 17), .foregroundColor: NSColor.darkGray
        ])
        ("演示贴图 · 拖动移动 · 双击关闭" as NSString).draw(at: NSPoint(x: 28, y: 32), withAttributes: [
            .font: NSFont.systemFont(ofSize: 12), .foregroundColor: NSColor.gray
        ])
        image.unlockFocus()
        return image
    }
}
