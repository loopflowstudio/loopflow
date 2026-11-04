import Foundation

/// A one-use local transfer. Never log the ticket or place it in a URL.
public struct MachinePairingTicket: Decodable, Sendable {
    public let version: Int
    public let endpoint: URL
    public let ticket: String

    public static func decode(_ data: Data) throws -> Self {
        guard data.count <= 4096 else { throw RegistryQueryError("This is not a Loopflow setup ticket") }
        let result = try JSONDecoder().decode(Self.self, from: data)
        guard result.version == 1, result.ticket.utf8.count == 64,
              result.ticket.utf8.allSatisfy({ (48...57).contains($0) || (65...70).contains($0) || (97...102).contains($0) }) else {
            throw RegistryQueryError("Create a new setup ticket with the current Machine service")
        }
        _ = try socketURL(endpoint: result.endpoint, path: "/v1/pair")
        return result
    }

    public static func socketURL(endpoint: URL, path: String) throws -> URL {
        guard var components = URLComponents(url: endpoint, resolvingAgainstBaseURL: false),
              components.scheme == "https", components.host?.hasSuffix(".ts.net") == true,
              components.user == nil, components.password == nil,
              components.query == nil, components.fragment == nil,
              components.path.isEmpty || components.path == "/" else {
            throw RegistryQueryError("Use the HTTPS address provided by this Machine’s Tailscale setup")
        }
        components.scheme = "wss"
        components.path = path
        guard let url = components.url else { throw RegistryQueryError("Invalid Machine address") }
        return url
    }
}
