import Foundation

public enum CaptureError: LocalizedError {
    case alreadyRunning
    case failed(String)

    public var errorDescription: String? {
        switch self {
        case .alreadyRunning: "已有一次截图正在进行。"
        case .failed(let message): message
        }
    }
}

/// Uses the system's selection UI, so Escape, multiple displays, and Retina
/// capture keep their native behavior. The child process never blocks AppKit.
@MainActor
public final class CaptureRunner {
    public private(set) var isRunning = false
    private var process: Process?
    private var captureDirectory: URL?
    private let executable: URL
    private let temporaryRoot: URL
    private let arguments: (@Sendable (URL) -> [String])?

    public init(
        executable: URL = URL(fileURLWithPath: "/usr/sbin/screencapture"),
        temporaryRoot: URL = FileManager.default.temporaryDirectory,
        arguments: (@Sendable (URL) -> [String])? = nil
    ) {
        self.executable = executable
        self.temporaryRoot = temporaryRoot
        self.arguments = arguments
    }

    public func capture() async throws -> Data? {
        guard !isRunning else { throw CaptureError.alreadyRunning }
        isRunning = true
        let directory = temporaryRoot.appendingPathComponent("snipsprig-\(UUID().uuidString)")
        captureDirectory = directory
        defer {
            try? FileManager.default.removeItem(at: directory)
            captureDirectory = nil
            process = nil
            isRunning = false
        }

        try FileManager.default.createDirectory(
            at: directory, withIntermediateDirectories: true,
            attributes: [.posixPermissions: 0o700]
        )
        let output = directory.appendingPathComponent("capture.png")
        let child = Process()
        let errorPipe = Pipe()
        child.executableURL = executable
        child.arguments = arguments?(output) ?? ["-i", "-s", "-x", "-t", "png", output.path]
        child.standardOutput = FileHandle.nullDevice
        child.standardError = errorPipe
        process = child

        let status: Int32 = try await withCheckedThrowingContinuation { continuation in
            child.terminationHandler = { finished in
                continuation.resume(returning: finished.terminationStatus)
            }
            do {
                try child.run()
            } catch {
                child.terminationHandler = nil
                continuation.resume(throwing: error)
            }
        }
        let diagnostic = String(
            data: errorPipe.fileHandleForReading.readDataToEndOfFile(), encoding: .utf8
        )?.trimmingCharacters(in: .whitespacesAndNewlines) ?? ""

        // Escape cancels the system picker and produces no file (usually exit 1).
        if !FileManager.default.fileExists(atPath: output.path) {
            if status == 0 || (status == 1 && diagnostic.isEmpty) { return nil }
            throw CaptureError.failed(diagnostic.isEmpty ? "系统截图失败，请检查屏幕录制权限。" : diagnostic)
        }
        guard status == 0 else { throw CaptureError.failed("系统截图未成功完成。") }
        return try Data(contentsOf: output)
    }

    public func cancel() {
        if process?.isRunning == true { process?.terminate() }
    }

    /// Synchronous shutdown cleanup; normal capture cleanup happens in `defer`.
    public func shutdown() {
        cancel()
        if let captureDirectory { try? FileManager.default.removeItem(at: captureDirectory) }
    }
}
