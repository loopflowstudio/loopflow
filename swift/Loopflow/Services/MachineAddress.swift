import Foundation

public enum MachineAddress {
    public static func decode(_ text: String) throws -> URL {
        let text = text.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !text.isEmpty, text.utf8.count <= 2048,
              !text.contains(where: { $0.isWhitespace }) else { throw invalidAddress() }
        let address = text.contains("://") ? text : "https://" + text
        guard var parts = URLComponents(string: address),
              parts.scheme?.lowercased() == "https",
              let host = parts.host?.lowercased(), host.hasSuffix(".ts.net"),
              host.split(separator: ".", omittingEmptySubsequences: false).allSatisfy({ label in
                  !label.isEmpty && label.utf8.allSatisfy { (97...122).contains($0) || (48...57).contains($0) || $0 == 45 }
              }),
              parts.user == nil, parts.password == nil,
              parts.query == nil, parts.fragment == nil,
              parts.path.isEmpty || parts.path == "/",
              parts.port == nil || (1...65535).contains(parts.port!) else { throw invalidAddress() }
        parts.scheme = "https"
        parts.host = host
        parts.path = ""
        guard let url = parts.url else { throw invalidAddress() }
        return url
    }

    public static func socketURL(endpoint: URL, path: String) throws -> URL {
        let endpoint = try decode(endpoint.absoluteString)
        guard var parts = URLComponents(url: endpoint, resolvingAgainstBaseURL: false) else { throw invalidAddress() }
        parts.scheme = "wss"
        parts.path = path
        guard let url = parts.url else { throw invalidAddress() }
        return url
    }

    private static func invalidAddress() -> RegistryQueryError {
        RegistryQueryError("Enter the Tailscale HTTPS address shown in Loopflow’s Mobile settings.")
    }
}
