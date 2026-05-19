import Foundation

public struct PairingPayload: Hashable, Sendable {
    public let host: String
    public let port: Int
    public let useTLS: Bool
    public let token: String
    public let fingerprint: String?

    public init(url: URL) throws {
        guard url.scheme == "loopflow", url.host == "pair" else {
            throw PairingPayloadError.invalidScheme
        }
        guard let components = URLComponents(url: url, resolvingAgainstBaseURL: false) else {
            throw PairingPayloadError.invalidURL
        }
        var items: [String: String] = [:]
        for item in components.queryItems ?? [] {
            guard let value = item.value else { continue }
            guard items[item.name] == nil else {
                throw PairingPayloadError.invalidField(item.name)
            }
            items[item.name] = value
        }

        guard let host = items["host"]?.trimmingCharacters(in: .whitespacesAndNewlines), !host.isEmpty else {
            throw PairingPayloadError.missingField("host")
        }
        guard let portValue = items["port"], let port = Int(portValue), (1 ... 65535).contains(port) else {
            throw PairingPayloadError.invalidField("port")
        }
        guard let tlsValue = items["tls"], let useTLS = Self.parseBool(tlsValue) else {
            throw PairingPayloadError.invalidField("tls")
        }
        guard let token = items["token"], !token.isEmpty else {
            throw PairingPayloadError.missingField("token")
        }

        let fingerprint = try items["fp"].map(Self.normalizeFingerprint)
        if !useTLS && !Self.isTailscaleIPv4(host) {
            throw PairingPayloadError.insecurePlaintextHost(host)
        }

        self.host = host
        self.port = port
        self.useTLS = useTLS
        self.token = token
        self.fingerprint = fingerprint
    }

    public var serverConnection: ServerConnection {
        ServerConnection(
            host: host,
            port: port,
            useTLS: useTLS,
            authMode: .staticToken,
            staticToken: token
        )
    }

    private static func parseBool(_ value: String) -> Bool? {
        switch value.lowercased() {
        case "true", "1": true
        case "false", "0": false
        default: nil
        }
    }

    private static func normalizeFingerprint(_ value: String) throws -> String {
        let normalized = value
            .filter { !$0.isWhitespace && $0 != ":" && $0 != "-" }
            .lowercased()
        guard normalized.count == 64, normalized.allSatisfy(\.isHexDigit) else {
            throw PairingPayloadError.invalidField("fp")
        }
        return normalized
    }

    private static func isTailscaleIPv4(_ host: String) -> Bool {
        let parts = host.split(separator: ".").compactMap { Int($0) }
        guard parts.count == 4, parts.allSatisfy({ (0 ... 255).contains($0) }) else {
            return false
        }
        return parts[0] == 100 && (64 ... 127).contains(parts[1])
    }
}

public enum PairingPayloadError: LocalizedError, Equatable, Sendable {
    case invalidScheme
    case invalidURL
    case missingField(String)
    case invalidField(String)
    case insecurePlaintextHost(String)

    public var errorDescription: String? {
        switch self {
        case .invalidScheme:
            return "Pairing links must start with loopflow://pair."
        case .invalidURL:
            return "Pairing link is not a valid URL."
        case .missingField(let field):
            return "Pairing link is missing \(field)."
        case .invalidField(let field):
            return "Pairing link has an invalid \(field)."
        case .insecurePlaintextHost(let host):
            return "Plaintext pairing is only allowed for Tailscale hosts; \(host) is not in 100.64.0.0/10."
        }
    }
}
