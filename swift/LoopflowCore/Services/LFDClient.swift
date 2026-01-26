// HTTP client for lfd daemon REST API.

import Foundation

public actor LFDClient {
    public static let shared = LFDClient()

    private let baseURL: URL
    private let session: URLSession
    private var _isAvailable: Bool?

    public init(baseURL: URL = URL(string: "http://127.0.0.1:8765")!) {
        self.baseURL = baseURL
        let config = URLSessionConfiguration.default
        config.timeoutIntervalForRequest = 2  // Fast fail if daemon not running
        config.timeoutIntervalForResource = 5
        self.session = URLSession(configuration: config)
    }

    public var isAvailable: Bool {
        _isAvailable ?? false
    }

    /// List worktrees via lfd HTTP endpoint.
    /// Returns nil if lfd is unavailable (caller should fall back to CLI).
    public func listWorktrees(repo: URL) async -> [WorktreeJSON]? {
        var components = URLComponents(url: baseURL, resolvingAgainstBaseURL: false)!
        components.path = "/worktrees"
        components.queryItems = [URLQueryItem(name: "repo", value: repo.path())]

        guard let url = components.url else {
            return nil
        }

        do {
            let (data, response) = try await session.data(from: url)

            guard let httpResponse = response as? HTTPURLResponse,
                  httpResponse.statusCode == 200 else {
                _isAvailable = false
                return nil
            }

            _isAvailable = true

            let decoded = try JSONDecoder().decode(LFDWorktreeResponse.self, from: data)
            guard decoded.ok, let result = decoded.result else {
                return nil
            }

            return result.worktrees
        } catch {
            // Connection refused, timeout, etc. - lfd not running
            _isAvailable = false
            LoggingService.append("lfd.http error=\(error.localizedDescription)", category: LoggingService.Category.lfd)
            return nil
        }
    }

    /// Check if lfd is available.
    public func checkAvailability() async -> Bool {
        var components = URLComponents(url: baseURL, resolvingAgainstBaseURL: false)!
        components.path = "/status"

        guard let url = components.url else {
            _isAvailable = false
            return false
        }

        do {
            let (_, response) = try await session.data(from: url)
            let available = (response as? HTTPURLResponse)?.statusCode == 200
            _isAvailable = available
            return available
        } catch {
            _isAvailable = false
            return false
        }
    }
}

// Response types matching lfd HTTP API

struct LFDWorktreeResponse: Codable {
    let ok: Bool
    let result: LFDWorktreeResult?
    let error: String?
}

struct LFDWorktreeResult: Codable {
    let worktrees: [WorktreeJSON]
}
