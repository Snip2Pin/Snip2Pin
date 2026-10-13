import Foundation
import Testing
@testable import SnipSprigKit

@Suite @MainActor struct CaptureRunnerTests {
    @Test func successfulCaptureReturnsBytesAndRemovesItsPrivateFile() async throws {
        let root = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        try FileManager.default.createDirectory(at: root, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: root) }
        let fixture = root.appendingPathComponent("fixture")
        let expected = Data([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a])
        try expected.write(to: fixture)
        let runner = CaptureRunner(executable: URL(fileURLWithPath: "/bin/cp"), temporaryRoot: root) {
            [fixture.path, $0.path]
        }
        #expect(try await runner.capture() == expected)
        #expect(!runner.isRunning)
        #expect(try FileManager.default.contentsOfDirectory(atPath: root.path) == ["fixture"])
    }

    @Test func cancelledCaptureReturnsNoImageAndDeletesTemporaryDirectory() async throws {
        let root = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        try FileManager.default.createDirectory(at: root, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: root) }
        let runner = CaptureRunner(executable: URL(fileURLWithPath: "/usr/bin/false"), temporaryRoot: root, arguments: { _ in [] })
        let data = try await runner.capture()
        #expect(data == nil)
        #expect(!runner.isRunning)
        #expect(try FileManager.default.contentsOfDirectory(atPath: root.path).isEmpty)
    }

    @Test func launchFailureCleansUpAndAllowsRetry() async throws {
        let root = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        try FileManager.default.createDirectory(at: root, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: root) }
        let runner = CaptureRunner(executable: root.appendingPathComponent("missing"), temporaryRoot: root)
        for _ in 0..<2 {
            do {
                _ = try await runner.capture()
                Issue.record("Missing executable must fail")
            } catch {
                #expect(!runner.isRunning)
                #expect(try FileManager.default.contentsOfDirectory(atPath: root.path).isEmpty)
            }
        }
    }

    @Test func simultaneousCaptureIsRejectedAndCancellationCleansUp() async throws {
        let root = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        try FileManager.default.createDirectory(at: root, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: root) }
        let runner = CaptureRunner(executable: URL(fileURLWithPath: "/bin/sleep"), temporaryRoot: root, arguments: { _ in ["10"] })
        let first = Task { try await runner.capture() }
        // The task gets the main actor before the next cooperative turn.
        while !runner.isRunning { await Task.yield() }
        do {
            _ = try await runner.capture()
            Issue.record("A second picker must not start")
        } catch CaptureError.alreadyRunning { }
        runner.cancel()
        _ = try? await first.value
        #expect(!runner.isRunning)
        #expect(try FileManager.default.contentsOfDirectory(atPath: root.path).isEmpty)
    }
}
