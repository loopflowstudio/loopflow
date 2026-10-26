// First launch of a copied Loopflow.app: its bundled helper activates the
// bundle as the machine installation (`lf install activate-app`) before the app
// reads the registry or resumes Sessions. Rust owns every installation decision;
// this only runs the helper, shows that it is running, and offers a retry.

#if os(macOS)
import Foundation
import Observation
import Loopflow

@MainActor
@Observable
final class AppActivation {
    enum State: Equatable {
        case activating
        case failed(String)
        case activated
    }

    private(set) var state: State
    private let activate: () async throws -> Void
    private var attempt: Task<Void, Error>?
    private var waiters: [CheckedContinuation<Void, Never>] = []

    /// `activate` nil means this launch has nothing to activate.
    init(activate: (() async throws -> Void)?) {
        self.activate = activate ?? {}
        state = activate == nil ? .activated : .activating
    }

    /// The launched app's activation: the packaged bundle's own helper. Tests,
    /// offline clones and unbundled development runs have nothing to activate.
    static let shared = AppActivation(activate: launchedBundleActivation())

    /// Activate, or join the attempt in flight; a failed attempt is retried.
    func activated() async throws {
        if state == .activated { return }
        if let attempt { return try await attempt.value }
        state = .activating
        let attempt = Task {
            defer { self.attempt = nil }
            do {
                try await activate()
            } catch {
                state = .failed(error.localizedDescription)
                throw error
            }
            state = .activated
            for waiter in waiters { waiter.resume() }
            waiters = []
        }
        self.attempt = attempt
        try await attempt.value
    }

    /// Wait for some attempt to succeed, however many fail first. Starts the
    /// first attempt when no window has.
    func untilActivated() async {
        if state == .activated { return }
        if attempt == nil, state == .activating { Task { try? await activated() } }
        await withCheckedContinuation { waiters.append($0) }
    }

    private static func launchedBundleActivation() -> (() async throws -> Void)? {
        let bundle = Bundle.main.bundleURL
        guard AppTestMode.current() == nil, !AppRuntime.isAutomatedTest,
              ProcessInfo.processInfo.environment["LF_CLONE_ROOT"] == nil,
              bundle.pathExtension == "app",
              let helper = Bundle.main.url(forAuxiliaryExecutable: "lf") else { return nil }
        return { try await runHelper(helper, bundle: bundle) }
    }

    /// Output goes to a file, not a pipe: promotion may quit and reopen this app
    /// while the helper continues, and a closed pipe would end the helper.
    private static func runHelper(_ helper: URL, bundle: URL) async throws {
        let log = FileManager.default.temporaryDirectory
            .appendingPathComponent("loopflow-activate-\(UUID().uuidString).log")
        FileManager.default.createFile(atPath: log.path, contents: nil)
        let output = try FileHandle(forWritingTo: log)
        defer {
            try? output.close()
            try? FileManager.default.removeItem(at: log)
        }
        let process = LocalWaveAgentLauncher.queryProcess(
            [helper.path, "install", "activate-app", "--bundle", bundle.path])
        process.standardInput = FileHandle.nullDevice
        process.standardOutput = output
        process.standardError = output
        let status: Int32 = try await withCheckedThrowingContinuation { continuation in
            process.terminationHandler = { continuation.resume(returning: $0.terminationStatus) }
            do { try process.run() } catch {
                process.terminationHandler = nil
                continuation.resume(throwing: LocalLfError(
                    errorDescription: "Could not start Loopflow setup: \(error.localizedDescription)"))
            }
        }
        guard status != 0 else { return }
        let detail = (try? String(contentsOf: log, encoding: .utf8))?
            .trimmingCharacters(in: .whitespacesAndNewlines) ?? ""
        throw LocalLfError(errorDescription: detail.isEmpty
            ? "Loopflow setup failed (exit \(status))." : detail)
    }
}
#endif
