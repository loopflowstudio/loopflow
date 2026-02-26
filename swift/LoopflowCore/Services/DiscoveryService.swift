import Foundation

public protocol StudioAuthTokenProvider: AnyObject {
    func currentToken() -> String?
    func tokenExpiresAt() -> Date?
    func refreshToken() async throws -> String
}

extension AuthService: StudioAuthTokenProvider {}

public enum DiscoveryServiceError: LocalizedError {
    case notAuthenticated
    case invalidResponse
    case httpStatus(Int)
    case invalidDaemonURL(String?)
    case missingConnectionToken(machineId: String)

    public var errorDescription: String? {
        switch self {
        case .notAuthenticated:
            return "Sign in to discover your running lfds."
        case .invalidResponse:
            return "Unexpected discovery response from loopflow.studio."
        case .httpStatus(let status):
            return "Discovery failed (HTTP \(status))."
        case .invalidDaemonURL:
            return "Daemon is missing a valid URL."
        case .missingConnectionToken:
            return "Daemon is missing a connection token."
        }
    }
}

public final class DiscoveryService: @unchecked Sendable {
    private let authService: StudioAuthTokenProvider
    private let baseURL: URL
    private let session: URLSession
    private let tokenRefreshLeadTime: TimeInterval

    public init(
        authService: StudioAuthTokenProvider,
        baseURL: URL = URL(string: "https://loopflow.studio")!,
        session: URLSession = .shared,
        tokenRefreshLeadTime: TimeInterval = 120
    ) {
        self.authService = authService
        self.baseURL = baseURL
        self.session = session
        self.tokenRefreshLeadTime = tokenRefreshLeadTime
    }

    public func discoverDaemons() async throws -> [DiscoveredDaemon] {
        let token = try await validToken()
        var request = URLRequest(url: baseURL.appendingPathComponent("api/v1/daemons/discover"))
        request.httpMethod = "GET"
        request.setValue("application/json", forHTTPHeaderField: "Accept")
        request.setValue("Bearer \(token)", forHTTPHeaderField: "Authorization")

        let (data, response) = try await session.data(for: request)
        guard let httpResponse = response as? HTTPURLResponse else {
            throw DiscoveryServiceError.invalidResponse
        }

        guard (200 ... 299).contains(httpResponse.statusCode) else {
            throw DiscoveryServiceError.httpStatus(httpResponse.statusCode)
        }

        do {
            return try Self.decoder().decode(DiscoverResponse.self, from: data).daemons
        } catch {
            throw DiscoveryServiceError.invalidResponse
        }
    }

    private func validToken() async throws -> String {
        guard let current = authService.currentToken() else {
            throw DiscoveryServiceError.notAuthenticated
        }

        if let expiry = authService.tokenExpiresAt(),
           expiry.timeIntervalSinceNow <= tokenRefreshLeadTime {
            return try await authService.refreshToken()
        }

        return current
    }

    private static func decoder() -> JSONDecoder {
        let decoder = JSONDecoder()
        decoder.keyDecodingStrategy = .convertFromSnakeCase
        decoder.dateDecodingStrategy = .custom(Self.decodeDate)
        return decoder
    }

    private static func decodeDate(from decoder: Decoder) throws -> Date {
        let container = try decoder.singleValueContainer()

        if let seconds = try? container.decode(Double.self) {
            return Date(timeIntervalSince1970: seconds)
        }
        if let secondsInt = try? container.decode(Int.self) {
            return Date(timeIntervalSince1970: TimeInterval(secondsInt))
        }
        if let raw = try? container.decode(String.self) {
            let fractional = ISO8601DateFormatter()
            fractional.formatOptions = [.withInternetDateTime, .withFractionalSeconds]
            if let date = fractional.date(from: raw) {
                return date
            }

            let standard = ISO8601DateFormatter()
            standard.formatOptions = [.withInternetDateTime]
            if let date = standard.date(from: raw) {
                return date
            }
        }

        throw DiscoveryServiceError.invalidResponse
    }
}

private struct DiscoverResponse: Decodable {
    let daemons: [DiscoveredDaemon]

    init(from decoder: Decoder) throws {
        let container = try decoder.singleValueContainer()
        if let daemons = try? container.decode([DiscoveredDaemon].self) {
            self.daemons = daemons
            return
        }

        let wrapped = try container.decode(WrappedPayload.self)
        self.daemons = wrapped.daemons ?? wrapped.data ?? []
    }

    private struct WrappedPayload: Decodable {
        let daemons: [DiscoveredDaemon]?
        let data: [DiscoveredDaemon]?
    }
}
