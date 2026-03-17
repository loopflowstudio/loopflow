import Foundation
import Security
import Darwin
import AppKit
import LoopflowCore

@MainActor
@Observable
final class BundledDaemonManager {
    enum DaemonState {
        case stopped
        case starting
        case running
        case failed(Error)
    }

    enum ManagerError: LocalizedError {
        case missingBundledExecutable(String)
        case failedToAllocatePort
        case failedToGenerateToken
        case healthCheckTimedOut(Int)
        case processExited(Int32)
        case dockerNotAvailable
        case containerStartFailed(Int32)
        case invalidRuntimeState

        var errorDescription: String? {
            switch self {
            case .missingBundledExecutable(let name):
                return "Missing bundled executable: \(name)"
            case .failedToAllocatePort:
                return "Failed to allocate a local port for lfd."
            case .failedToGenerateToken:
                return "Failed to generate session token for bundled lfd."
            case .healthCheckTimedOut(let port):
                return "Bundled lfd did not become healthy on port \(port)."
            case .processExited(let code):
                return "Bundled lfd exited early with status \(code)."
            case .dockerNotAvailable:
                return "Docker is not running. Install Docker Desktop or start the Docker daemon."
            case .containerStartFailed(let code):
                return "Failed to start lfd container (exit code \(code))."
            case .invalidRuntimeState:
                return "Bundled lfd started but runtime connection details are missing."
            }
        }
    }

    private let executableProvider: @Sendable (String) -> URL?
    private let urlSession: URLSession

    private var process: Process?
    private var containerName: String?
    private var credentialServer: CredentialSocketServer?
    private var terminationObserver: NSObjectProtocol?
    private var runningSettings: RuntimeSettings?

    private(set) var port: Int?
    private(set) var token: String?
    private(set) var state: DaemonState = .stopped
    private var startTask: Task<ServerConnection, Error>?

    private static let preferNativeModeDefaultsKey = "concerto.bundledDaemon.preferNativeMode"
    private static let connectWithPhoneDefaultsKey = "concerto.bundledDaemon.connectWithPhone"

    static var prefersNativeMode: Bool {
        UserDefaults.standard.bool(forKey: preferNativeModeDefaultsKey)
    }

    static func setPreferNativeMode(_ enabled: Bool) {
        UserDefaults.standard.set(enabled, forKey: preferNativeModeDefaultsKey)
    }

    static var connectWithPhoneEnabled: Bool {
        UserDefaults.standard.bool(forKey: connectWithPhoneDefaultsKey)
    }

    static func setConnectWithPhoneEnabled(_ enabled: Bool) {
        UserDefaults.standard.set(enabled, forKey: connectWithPhoneDefaultsKey)
    }

    init(
        executableProvider: @escaping @Sendable (String) -> URL? = { name in
            Bundle.main.url(forAuxiliaryExecutable: name)
        },
        urlSession: URLSession = .shared
    ) {
        self.executableProvider = executableProvider
        self.urlSession = urlSession

        terminationObserver = NotificationCenter.default.addObserver(
            forName: NSApplication.willTerminateNotification,
            object: nil,
            queue: nil
        ) { [weak self] _ in
            Task { @MainActor in
                self?.stop()
            }
        }
    }

    /// Start the daemon eagerly. Later calls to `start()` will join this in-flight task.
    func eagerStart() {
        guard startTask == nil else { return }
        startTask = Task {
            try await performStart()
        }
    }

    func start() async throws -> ServerConnection {
        // Join an in-flight eager start if one exists.
        if let task = startTask {
            return try await task.value
        }

        let task = Task {
            try await performStart()
        }
        startTask = task
        return try await task.value
    }

    private func performStart() async throws -> ServerConnection {
        let settings = RuntimeSettings(
            preferNativeMode: Self.prefersNativeMode,
            connectWithPhone: Self.connectWithPhoneEnabled
        )

        if case .running = state,
           runningSettings == settings,
           let connection = runtimeConnection {
            return connection
        }

        if case .running = state {
            stop()
        }

        state = .starting

        do {
            let token = try generateSessionToken()
            let port = try allocatePort()

            if settings.preferNativeMode {
                try startNativeDaemon(
                    token: token,
                    port: port,
                    connectWithPhone: settings.connectWithPhone
                )
            } else if try await dockerAvailable() {
                do {
                    try await startContainerDaemon(
                        token: token,
                        port: port,
                        connectWithPhone: settings.connectWithPhone
                    )
                } catch {
                    try startNativeDaemon(
                        token: token,
                        port: port,
                        connectWithPhone: settings.connectWithPhone
                    )
                }
            } else {
                try startNativeDaemon(
                    token: token,
                    port: port,
                    connectWithPhone: settings.connectWithPhone
                )
            }

            self.token = token
            self.port = port
            self.runningSettings = settings

            try await waitForHealth()
            let connection = try requireRuntimeConnection()
            state = .running
            return connection
        } catch {
            startTask = nil
            stop()
            state = .failed(error)
            throw error
        }
    }

    func stop() {
        startTask = nil
        state = .stopped

        if let process {
            if process.isRunning {
                process.terminate()
                let deadline = Date().addingTimeInterval(3)
                while process.isRunning && Date() < deadline {
                    Thread.sleep(forTimeInterval: 0.05)
                }

                if process.isRunning {
                    kill(process.processIdentifier, SIGKILL)
                    process.waitUntilExit()
                }
            }
        }

        if let name = containerName, let docker = try? dockerExecutableURL() {
            _ = try? runProcess(docker, arguments: ["stop", "-t", "3", name])
            _ = try? runProcess(docker, arguments: ["rm", "-f", name])
        }

        credentialServer?.stop()
        resetRuntime()
    }

    private func startNativeDaemon(token: String, port: Int, connectWithPhone: Bool) throws {
        let dbPath = try sqlitePath()
        let process = Process()
        guard let lfdPath = executableProvider("lfd") else {
            throw ManagerError.missingBundledExecutable("lfd")
        }

        process.executableURL = lfdPath
        process.arguments = connectWithPhone ? ["serve", "--allow-insecure-bind"] : ["serve"]

        var env = ProcessInfo.processInfo.environment
        env["LFD_HTTP_ADDR"] = connectWithPhone ? "0.0.0.0:\(port)" : "127.0.0.1:\(port)"
        env["LFD_DB_PATH"] = dbPath.path
        env["LFD_AUTH_MODE"] = connectWithPhone ? "studio" : "local"
        env["LFD_AUTH_MODE"] = connectWithPhone ? "studio" : "local"
        env["LFD_AUTH_TOKEN"] = token
        process.environment = env

        process.terminationHandler = { [weak self] process in
            Task { @MainActor in
                guard let self else { return }
                if case .stopped = self.state {
                    self.resetRuntime()
                    return
                }
                self.state = .failed(ManagerError.processExited(process.terminationStatus))
                self.resetRuntime()
            }
        }

        try process.run()
        self.process = process
    }

    private func startContainerDaemon(
        token: String,
        port: Int,
        connectWithPhone: Bool
    ) async throws {
        guard try await dockerAvailable() else {
            throw ManagerError.dockerNotAvailable
        }

        let credentialServer = CredentialSocketServer()
        try credentialServer.start()
        let containerName = "concerto-lfd-\(String(token.prefix(8)))"
        let docker = try dockerExecutableURL()
        let image = lfdContainerImage()

        let pullResult = try runProcess(docker, arguments: ["pull", image])
        guard pullResult.terminationStatus == 0 else {
            credentialServer.stop()
            throw ManagerError.containerStartFailed(pullResult.terminationStatus)
        }

        let args = buildDockerRunArgs(
            containerName: containerName,
            port: port,
            token: token,
            credentialSocketPath: credentialServer.socketPath,
            srcPath: srcDirectory(),
            connectWithPhone: connectWithPhone
        )
        let runResult = try runProcess(docker, arguments: args)
        guard runResult.terminationStatus == 0 else {
            credentialServer.stop()
            throw ManagerError.containerStartFailed(runResult.terminationStatus)
        }

        self.credentialServer = credentialServer
        self.containerName = containerName
    }

    private var runtimeConnection: ServerConnection? {
        guard let port, let token else { return nil }
        return ServerConnection(
            host: "127.0.0.1",
            port: port,
            useTLS: false,
            authMode: .staticToken,
            staticToken: token
        )
    }

    var currentConnection: ServerConnection? {
        runtimeConnection
    }

    private func requireRuntimeConnection() throws -> ServerConnection {
        guard let connection = runtimeConnection else {
            throw ManagerError.invalidRuntimeState
        }
        return connection
    }

    private func waitForHealth(timeout: TimeInterval = 10) async throws {
        guard let port else { throw ManagerError.invalidRuntimeState }

        let deadline = Date().addingTimeInterval(timeout)
        while Date() < deadline {
            if process?.isRunning == false {
                throw ManagerError.processExited(process?.terminationStatus ?? -1)
            }

            let url = URL(string: "http://127.0.0.1:\(port)/health")!
            var request = URLRequest(url: url)
            request.timeoutInterval = 0.25

            do {
                let (_, response) = try await urlSession.data(for: request)
                if let http = response as? HTTPURLResponse, http.statusCode == 200 {
                    return
                }
            } catch {
                // Keep polling until timeout or success.
            }

            try await Task.sleep(for: .milliseconds(50))
        }

        throw ManagerError.healthCheckTimedOut(port)
    }

    private func lfdContainerImage() -> String {
        let configured = loadConcertoConfig()?.container?.image?
            .trimmingCharacters(in: .whitespacesAndNewlines)
        if let configured, !configured.isEmpty {
            return configured
        }
        return "loopflow/lfd:latest"
    }

    private func dockerExecutableURL() throws -> URL {
        let candidates = [
            "/usr/local/bin/docker",
            "/opt/homebrew/bin/docker",
            "/usr/bin/docker",
        ]
        for path in candidates where FileManager.default.isExecutableFile(atPath: path) {
            return URL(fileURLWithPath: path)
        }
        throw ManagerError.dockerNotAvailable
    }

    private func runProcess(_ executableURL: URL, arguments: [String]) throws -> ProcessResult {
        let process = Process()
        process.executableURL = executableURL
        process.arguments = arguments
        process.standardOutput = FileHandle.nullDevice
        process.standardError = FileHandle.nullDevice
        try process.run()
        process.waitUntilExit()
        return ProcessResult(terminationStatus: process.terminationStatus)
    }

    private func buildDockerRunArgs(
        containerName: String,
        port: Int,
        token: String,
        credentialSocketPath: URL,
        srcPath: URL,
        connectWithPhone: Bool
    ) -> [String] {
        let portMapping = connectWithPhone ? "\(port):2486" : "127.0.0.1:\(port):2486"
        let authMode = connectWithPhone ? "studio" : "local"
        let authMode = connectWithPhone ? "studio" : "local"
        var args = [
            "run", "-d",
            "--name", containerName,
            "-p", portMapping,
            "-v", "\(srcPath.path):/workspace/src:ro",
            "-v", "/var/run/docker.sock:/var/run/docker.sock",
            "-v", "concerto-lfd-data:/data",
            "-v", "\(credentialSocketPath.path):/var/run/concerto-auth.sock:ro",
            "-e", "LFD_HTTP_ADDR=0.0.0.0:2486",
            "-e", "LFD_DB_PATH=/data/concerto.db",
            "-e", "LFD_AUTH_MODE=\(authMode)",
            "-e", "LFD_AUTH_TOKEN=\(token)",
            "-e", "LFD_CREDENTIAL_SOCKET=/var/run/concerto-auth.sock",
            "-e", "LFD_MODE=container",
        ]

        let home = FileManager.default.homeDirectoryForCurrentUser
        let sshDir = home.appendingPathComponent(".ssh")
        if FileManager.default.fileExists(atPath: sshDir.path) {
            args += ["-v", "\(sshDir.path):/root/.ssh:ro"]
        }
        let gitconfig = home.appendingPathComponent(".gitconfig")
        if FileManager.default.fileExists(atPath: gitconfig.path) {
            args += ["-v", "\(gitconfig.path):/root/.gitconfig:ro"]
        }

        if let mounts = loadConcertoConfig()?.container?.mounts {
            for mount in mounts {
                let expandedPath = (mount.path as NSString).expandingTildeInPath
                let mode = mount.readOnly ? "ro" : "rw"
                let name = URL(fileURLWithPath: expandedPath).lastPathComponent
                args += ["-v", "\(expandedPath):/workspace/extra/\(name):\(mode)"]
            }
        }

        args += connectWithPhone
            ? [lfdContainerImage(), "serve", "--allow-insecure-bind"]
            : [lfdContainerImage(), "serve"]
        return args
    }

    private func dockerAvailable() async throws -> Bool {
        let docker = try dockerExecutableURL()
        let result = try runProcess(docker, arguments: ["info", "--format", "{{.ServerVersion}}"])
        return result.terminationStatus == 0
    }

    private func srcDirectory() -> URL {
        FileManager.default.homeDirectoryForCurrentUser.appendingPathComponent("src")
    }

    private func sqlitePath() throws -> URL {
        let supportRoot = FileManager.default.urls(for: .applicationSupportDirectory, in: .userDomainMask)
            .first?
            .appendingPathComponent("Concerto", isDirectory: true)
            .appendingPathComponent("lfd", isDirectory: true)
        guard let supportRoot else {
            throw ManagerError.invalidRuntimeState
        }

        try FileManager.default.createDirectory(at: supportRoot, withIntermediateDirectories: true)
        return supportRoot.appendingPathComponent("concerto.db", isDirectory: false)
    }

    private func generateSessionToken() throws -> String {
        var bytes = [UInt8](repeating: 0, count: 32)
        let status = SecRandomCopyBytes(kSecRandomDefault, bytes.count, &bytes)
        guard status == errSecSuccess else {
            throw ManagerError.failedToGenerateToken
        }
        return bytes.map { String(format: "%02x", $0) }.joined()
    }

    private func allocatePort() throws -> Int {
        let socketFD = socket(AF_INET, SOCK_STREAM, 0)
        guard socketFD >= 0 else {
            throw ManagerError.failedToAllocatePort
        }
        defer { close(socketFD) }

        var address = sockaddr_in()
        address.sin_len = UInt8(MemoryLayout<sockaddr_in>.size)
        address.sin_family = sa_family_t(AF_INET)
        address.sin_port = in_port_t(0).bigEndian
        address.sin_addr = in_addr(s_addr: inet_addr("127.0.0.1"))

        let bindResult = withUnsafePointer(to: &address) { pointer in
            pointer.withMemoryRebound(to: sockaddr.self, capacity: 1) { addr in
                bind(socketFD, addr, socklen_t(MemoryLayout<sockaddr_in>.size))
            }
        }
        guard bindResult == 0 else {
            throw ManagerError.failedToAllocatePort
        }

        var assignedAddress = sockaddr_in()
        var length = socklen_t(MemoryLayout<sockaddr_in>.size)
        let nameResult = withUnsafeMutablePointer(to: &assignedAddress) { pointer in
            pointer.withMemoryRebound(to: sockaddr.self, capacity: 1) { addr in
                getsockname(socketFD, addr, &length)
            }
        }
        guard nameResult == 0 else {
            throw ManagerError.failedToAllocatePort
        }

        let port = Int(UInt16(bigEndian: assignedAddress.sin_port))
        guard port > 0 else {
            throw ManagerError.failedToAllocatePort
        }
        return port
    }

    private func resetRuntime() {
        process = nil
        containerName = nil
        credentialServer = nil
        port = nil
        token = nil
        runningSettings = nil
    }
}

private struct ProcessResult {
    let terminationStatus: Int32
}

private struct RuntimeSettings: Equatable {
    let preferNativeMode: Bool
    let connectWithPhone: Bool
}
