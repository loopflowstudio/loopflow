#if os(macOS)
import Foundation
import Testing
@testable import LoopflowMac

/// A source-built server in a private Machine; never registers a system service.
actor LocalServerFixture {
    let lf: BuiltLf
    /// Where the server's debug diagnostics go, when a test counts them.
    let diagnostics: URL?
    private var process: Process?
    private var exits: AsyncStream<Void>?
    private var startup: Task<String, any Error>?

    init(_ lf: BuiltLf, diagnostics: URL? = nil) {
        self.lf = lf
        self.diagnostics = diagnostics
    }

    deinit { if let process, process.isRunning { process.terminate() } }

    nonisolated func location() -> ServerLocation {
        ServerLocation { _ in try await self.status() }
    }

    func status() async throws -> String {
        if let startup { return try await startup.value }
        let child = lf.process(["server", "run"])
        child.standardInput = FileHandle.nullDevice
        child.standardOutput = FileHandle.nullDevice
        if let diagnostics {
            child.environment = lf.environment.merging(["RUST_LOG": "loopflow::server=debug"]) { $1 }
            child.standardError = try FileHandle(forWritingTo: diagnostics)
        } else {
            child.standardError = FileHandle.nullDevice
        }
        let (exits, ended) = AsyncStream<Void>.makeStream()
        child.terminationHandler = { _ in ended.finish() }
        try child.run()
        process = child
        self.exits = exits
        let task = Task { [lf] in
            let deadline = ContinuousClock.now + .seconds(20)
            while true {
                let output = try await lf.run(["server", "status"])
                let value = try #require(JSONSerialization.jsonObject(with: Data(output.utf8)) as? [String: Any])
                if value["ready"] as? Bool == true { return output }
                try #require(ContinuousClock.now < deadline, "Private server did not become ready: \(output)")
                try await Task.sleep(for: .milliseconds(20))
            }
        }
        startup = task
        return try await task.value
    }

    var processID: Int32? { process?.processIdentifier }

    /// Connections the server admitted so far, from its debug diagnostics.
    nonisolated func admittedConnections() throws -> Int {
        let log = try String(contentsOf: #require(diagnostics), encoding: .utf8)
        return log.components(separatedBy: "Server connection admitted").count - 1
    }

    func stop() async {
        let process = process
        let exits = exits
        self.process = nil
        self.exits = nil
        startup = nil
        if let process, process.isRunning { process.terminate() }
        if let exits { for await _ in exits {} }
    }
}
#endif
