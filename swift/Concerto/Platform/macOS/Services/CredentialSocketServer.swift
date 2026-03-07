import AppKit
import Darwin
import Foundation
import Security

// SAFETY: lifecycle methods (`start`/`stop`) are called from the main app
// thread; the background listener thread only touches per-connection locals.
final class CredentialSocketServer: @unchecked Sendable {
    let socketPath: URL
    let homeDirectory: URL

    private var listenerThread: Thread?
    private var listenerFD: Int32 = -1

    static let defaultPath = FileManager.default.temporaryDirectory
        .appendingPathComponent("concerto-auth-\(ProcessInfo.processInfo.processIdentifier).sock")

    init(
        socketPath: URL = CredentialSocketServer.defaultPath,
        homeDirectory: URL = FileManager.default.homeDirectoryForCurrentUser
    ) {
        self.socketPath = socketPath
        self.homeDirectory = homeDirectory
    }

    func start() throws {
        stop()
        try? FileManager.default.removeItem(at: socketPath)

        let fd = socket(AF_UNIX, SOCK_STREAM, 0)
        guard fd >= 0 else {
            throw POSIXError(.ENOTSUP)
        }

        var address = sockaddr_un()
        address.sun_family = sa_family_t(AF_UNIX)
        let pathBytes = Array(socketPath.path.utf8)
        guard pathBytes.count < MemoryLayout.size(ofValue: address.sun_path) else {
            close(fd)
            throw POSIXError(.ENAMETOOLONG)
        }
        withUnsafeMutableBytes(of: &address.sun_path) { rawBuffer in
            rawBuffer.initializeMemory(as: CChar.self, repeating: 0)
            for (index, byte) in pathBytes.enumerated() {
                rawBuffer[index] = byte
            }
        }

        let addressLength = socklen_t(MemoryLayout<sa_family_t>.size + pathBytes.count + 1)
        let bindResult = withUnsafePointer(to: &address) { pointer in
            pointer.withMemoryRebound(to: sockaddr.self, capacity: 1) { sockaddrPtr in
                bind(fd, sockaddrPtr, addressLength)
            }
        }
        guard bindResult == 0 else {
            close(fd)
            throw POSIXError(POSIXErrorCode(rawValue: errno) ?? .EINVAL)
        }

        guard listen(fd, 8) == 0 else {
            close(fd)
            throw POSIXError(POSIXErrorCode(rawValue: errno) ?? .EINVAL)
        }

        chmod(socketPath.path, 0o600)
        listenerFD = fd
        let thread = Thread { [weak self] in
            self?.acceptLoop(listenerFD: fd)
        }
        thread.qualityOfService = .utility
        thread.start()
        listenerThread = thread
    }

    func stop() {
        listenerThread?.cancel()
        listenerThread = nil

        if listenerFD >= 0 {
            shutdown(listenerFD, SHUT_RDWR)
            close(listenerFD)
            listenerFD = -1
        }

        try? FileManager.default.removeItem(at: socketPath)
    }

    private func acceptLoop(listenerFD: Int32) {
        while true {
            if Thread.current.isCancelled { break }
            var clientAddress = sockaddr()
            var clientLength = socklen_t(MemoryLayout<sockaddr>.size)
            let clientFD = withUnsafeMutablePointer(to: &clientAddress) { pointer in
                accept(listenerFD, pointer, &clientLength)
            }

            if clientFD < 0 {
                if errno == EINTR { continue }
                if errno == EBADF || errno == EINVAL { break }
                continue
            }

            handleClient(clientFD)
            close(clientFD)
        }
    }

    private func handleClient(_ clientFD: Int32) {
        guard let request = readRequest(clientFD) else {
            writeResponse(clientFD, statusCode: 400, jsonBody: ["error": "invalid request"])
            return
        }

        let route = request.path.split(separator: "?").first.map(String.init) ?? request.path

        if request.method == "GET", route == "/health" {
            writeRawResponse(clientFD, statusCode: 200, contentType: "text/plain", body: Data("ok".utf8))
            return
        }

        if request.method == "GET", let provider = routeProvider(route, prefix: "/credentials/") {
            if let credential = readCredential(provider: provider) {
                writeJSON(clientFD, statusCode: 200, payload: credential)
            } else {
                writeResponse(clientFD, statusCode: 404, jsonBody: ["error": "credential not found"])
            }
            return
        }

        if request.method == "DELETE", let provider = routeProvider(route, prefix: "/credentials/") {
            _ = deleteKeychain(provider: provider)
            writeRawResponse(clientFD, statusCode: 204, contentType: "application/json", body: Data())
            return
        }

        if request.method == "POST", let provider = routeProvider(route, prefix: "/auth/"), route.hasSuffix("/start") {
            writeJSON(clientFD, statusCode: 200, payload: startAuth(provider: provider))
            return
        }

        writeResponse(clientFD, statusCode: 404, jsonBody: ["error": "not found"])
    }

    private func routeProvider(_ path: String, prefix: String) -> CredentialProvider? {
        guard path.hasPrefix(prefix) else { return nil }
        let suffix = String(path.dropFirst(prefix.count))
        let providerName = suffix.replacingOccurrences(of: "/start", with: "")
        let normalized = providerName.trimmingCharacters(in: .whitespacesAndNewlines).lowercased()
        return CredentialProvider(rawValue: normalized)
    }

    private func readRequest(_ fd: Int32) -> HTTPRequest? {
        var data = Data()
        let headerDelimiter = Data("\r\n\r\n".utf8)

        while true {
            var buffer = [UInt8](repeating: 0, count: 4096)
            let bytesRead = Darwin.read(fd, &buffer, buffer.count)
            if bytesRead <= 0 {
                break
            }
            data.append(buffer, count: Int(bytesRead))

            guard let headerRange = data.range(of: headerDelimiter) else {
                continue
            }

            let headerData = data.subdata(in: 0 ..< headerRange.lowerBound)
            guard let headerText = String(data: headerData, encoding: .utf8) else {
                return nil
            }
            let lines = headerText.components(separatedBy: "\r\n")
            guard let requestLine = lines.first else {
                return nil
            }

            let parts = requestLine.split(separator: " ")
            guard parts.count >= 2 else {
                return nil
            }
            let method = String(parts[0]).uppercased()
            let path = String(parts[1])
            return HTTPRequest(method: method, path: path)
        }

        return nil
    }

    private func writeJSON<T: Encodable>(_ fd: Int32, statusCode: Int, payload: T) {
        guard let data = try? JSONEncoder().encode(payload) else {
            writeResponse(fd, statusCode: 500, jsonBody: ["error": "encoding failed"])
            return
        }
        writeRawResponse(fd, statusCode: statusCode, contentType: "application/json", body: data)
    }

    private func writeResponse(_ fd: Int32, statusCode: Int, jsonBody: [String: String]) {
        let data = (try? JSONSerialization.data(withJSONObject: jsonBody)) ?? Data("{}".utf8)
        writeRawResponse(fd, statusCode: statusCode, contentType: "application/json", body: data)
    }

    private func writeRawResponse(_ fd: Int32, statusCode: Int, contentType: String, body: Data) {
        let reason = reasonPhrase(for: statusCode)
        let header = """
        HTTP/1.1 \(statusCode) \(reason)\r
        Content-Type: \(contentType)\r
        Content-Length: \(body.count)\r
        Connection: close\r
        \r
        """

        _ = header.withCString { ptr in
            Darwin.write(fd, ptr, strlen(ptr))
        }
        _ = body.withUnsafeBytes { buffer in
            guard let baseAddress = buffer.baseAddress else { return 0 }
            return Darwin.write(fd, baseAddress, buffer.count)
        }
    }

    private func reasonPhrase(for statusCode: Int) -> String {
        switch statusCode {
        case 200: return "OK"
        case 201: return "Created"
        case 204: return "No Content"
        case 400: return "Bad Request"
        case 404: return "Not Found"
        default: return "Internal Server Error"
        }
    }

    private func startAuth(provider: CredentialProvider) -> CredentialAuthStartResponse {
        if let url = URL(string: provider.verificationURI) {
            NSWorkspace.shared.open(url)
        }

        return CredentialAuthStartResponse(
            verification_uri: provider.verificationURI,
            verification_uri_complete: nil,
            user_code: nil,
            expires_in: nil
        )
    }

    private func readCredential(provider: CredentialProvider) -> CredentialResponse? {
        FileCredentialReader(homeDirectory: homeDirectory).read(provider: provider)
            ?? readKeychain(provider: provider)
    }

    private func readKeychain(provider: CredentialProvider) -> CredentialResponse? {
        let service = provider.keychainService
        let account = provider.keychainAccount

        var query: [String: Any] = [
            kSecClass as String: kSecClassGenericPassword,
            kSecAttrService as String: service,
            kSecReturnData as String: true,
            kSecMatchLimit as String: kSecMatchLimitOne,
        ]
        if let account {
            query[kSecAttrAccount as String] = account
        }

        var result: AnyObject?
        let status = SecItemCopyMatching(query as CFDictionary, &result)
        guard status == errSecSuccess, let data = result as? Data,
              let token = String(data: data, encoding: .utf8) else {
            return nil
        }

        return CredentialResponse(token: token, login: nil, expires_at: nil)
    }

    private func deleteKeychain(provider: CredentialProvider) -> Bool {
        let service = provider.keychainService
        let account = provider.keychainAccount

        var query: [String: Any] = [
            kSecClass as String: kSecClassGenericPassword,
            kSecAttrService as String: service,
        ]
        if let account {
            query[kSecAttrAccount as String] = account
        }

        let status = SecItemDelete(query as CFDictionary)
        return status == errSecSuccess || status == errSecItemNotFound
    }
}

struct FileCredentialReader {
    let homeDirectory: URL

    func read(provider: CredentialProvider) -> CredentialResponse? {
        switch provider {
        case .github:
            return nil
        case .claude:
            return readClaudeCredential()
        case .codex:
            return readCodexCredential()
        }
    }

    private func readClaudeCredential() -> CredentialResponse? {
        let path = homeDirectory
            .appendingPathComponent(".claude", isDirectory: true)
            .appendingPathComponent(".credentials.json")
        guard
            let json = readJSONObject(at: path),
            let token = json["accessToken"] as? String,
            !token.isEmpty
        else {
            return nil
        }

        let login = json["email"] as? String
        let expiresAt = firstString(
            from: json,
            keys: ["expiresAt", "expires_at", "accessTokenExpiresAt", "access_token_expires_at"]
        )
        return CredentialResponse(token: token, login: login, expires_at: expiresAt)
    }

    private func readCodexCredential() -> CredentialResponse? {
        let path = homeDirectory
            .appendingPathComponent(".codex", isDirectory: true)
            .appendingPathComponent("auth.json")
        guard
            let json = readJSONObject(at: path),
            let token = codexAccessToken(from: json),
            !token.isEmpty
        else {
            return nil
        }

        let expiresAt = firstString(
            from: json,
            keys: ["expires_at", "expiresAt", "accessTokenExpiresAt", "access_token_expires_at"]
        )
        return CredentialResponse(token: token, login: nil, expires_at: expiresAt)
    }

    private func codexAccessToken(from json: [String: Any]) -> String? {
        if let token = json["access_token"] as? String {
            return token
        }

        let nested = json["tokens"] as? [String: Any]
        return nested?["access_token"] as? String
    }

    private func readJSONObject(at url: URL) -> [String: Any]? {
        guard
            let data = try? Data(contentsOf: url),
            let object = try? JSONSerialization.jsonObject(with: data) as? [String: Any]
        else {
            return nil
        }
        return object
    }

    private func firstString(from json: [String: Any], keys: [String]) -> String? {
        for key in keys {
            guard let value = json[key] else { continue }
            if let string = value as? String, !string.isEmpty {
                return string
            }
            if let number = value as? NSNumber {
                return number.stringValue
            }
        }
        return nil
    }
}

private struct HTTPRequest {
    let method: String
    let path: String
}

enum CredentialProvider: String {
    case github
    case claude
    case codex

    var verificationURI: String {
        switch self {
        case .github:
            return "https://github.com/login/device"
        case .claude:
            return "https://console.anthropic.com/settings/keys"
        case .codex:
            return "https://platform.openai.com/api-keys"
        }
    }

    var keychainService: String {
        switch self {
        case .github:
            return "gh:github.com"
        case .claude:
            return "Claude Safe Storage"
        case .codex:
            return "Codex Safe Storage"
        }
    }

    var keychainAccount: String? {
        switch self {
        case .github:
            return nil
        case .claude:
            return "Claude"
        case .codex:
            return "Codex"
        }
    }
}

private struct CredentialAuthStartResponse: Codable {
    let verification_uri: String
    let verification_uri_complete: String?
    let user_code: String?
    let expires_in: Int?
}

struct CredentialResponse: Codable {
    let token: String
    let login: String?
    let expires_at: String?
}
