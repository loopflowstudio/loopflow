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
        case invalidRuntimeState

        var errorDescription: String? {
            switch self {
            case .missingBundledExecutable(let name):
                return "Missing bundled executable: \(name)"
            case .failedToAllocatePort:
                return "Failed to allocate a local port for lfd."
            case .failedToGenerateToken:
                return "Failed to generate auth token for bundled lfd."
            case .healthCheckTimedOut(let port):
                return "Bundled lfd did not become healthy on port \(port)."
            case .processExited(let code):
                return "Bundled lfd exited early with status \(code)."
            case .invalidRuntimeState:
                return "Bundled lfd started but runtime connection details are missing."
            }
        }
    }

    private let executableProvider: @Sendable (String) -> URL?
    private let urlSession: URLSession
    private var process: Process?
    private var terminationObserver: NSObjectProtocol?

    private(set) var port: Int?
    private(set) var token: String?
    private(set) var state: DaemonState = .stopped

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

    func start() async throws -> ServerConnection {
        if case .running = state, let connection = runtimeConnection {
            return connection
        }

        if case .starting = state {
            try await waitForHealth()
            let connection = try requireRuntimeConnection()
            state = .running
            return connection
        }

        state = .starting

        do {
            let token = try generateSessionToken()
            let port = try allocatePort()
            let dbPath = try sqlitePath()

            let process = Process()
            guard let lfdPath = executableProvider("lfd") else {
                throw ManagerError.missingBundledExecutable("lfd")
            }
            process.executableURL = lfdPath
            process.arguments = ["serve"]

            var env = ProcessInfo.processInfo.environment
            env["LFD_HTTP_ADDR"] = "127.0.0.1:\(port)"
            env["LFD_DB_PATH"] = dbPath.path
            env["LFD_AUTH_PROVIDER"] = "static"
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
            self.token = token
            self.port = port

            try await waitForHealth()
            let connection = try requireRuntimeConnection()
            state = .running
            return connection
        } catch {
            stop()
            state = .failed(error)
            throw error
        }
    }

    func stop() {
        state = .stopped

        guard let process else {
            resetRuntime()
            return
        }

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

        resetRuntime()
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
            request.timeoutInterval = 0.75

            do {
                let (_, response) = try await urlSession.data(for: request)
                if let http = response as? HTTPURLResponse, http.statusCode == 200 {
                    return
                }
            } catch {
                // Keep polling until timeout or success.
            }

            try await Task.sleep(for: .milliseconds(120))
        }

        throw ManagerError.healthCheckTimedOut(port)
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
        port = nil
        token = nil
    }
}

