import Foundation

public enum PortfolioMode: String, Codable, Sendable {
    case folder, manual
}

public struct PortfolioRepo: Codable, Identifiable, Hashable, Sendable {
    public let path: String
    public let error: String?

    public init(path: String, error: String?) {
        self.path = path
        self.error = error
    }
    public var id: String { path }
    public var url: URL { URL(fileURLWithPath: path) }
    public var displayName: String { url.lastPathComponent }
}

public struct Portfolio: Codable, Equatable, Sendable {
    public let configured: Bool
    public let mode: PortfolioMode
    public let root: String
    public let repos: [PortfolioRepo]
    public let error: String?
}
