import Foundation
import XCTest

@testable import LoopflowCore

final class DiscoveryServiceTests: XCTestCase {
    override func tearDown() {
        super.tearDown()
        DiscoveryStubURLProtocol.handler = nil
    }

    func testDiscoverDaemonsDecodesArrayPayload() async throws {
        let auth = MockAuthTokenProvider(token: "jwt-token", expiry: Date().addingTimeInterval(3600))
        DiscoveryStubURLProtocol.handler = { request in
            XCTAssertEqual(request.url?.path, "/api/v1/daemons/discover")
            XCTAssertEqual(request.value(forHTTPHeaderField: "Authorization"), "Bearer jwt-token")

            let body = """
            [
              {
                "machine_id": "machine-1",
                "machine_name": "jacks-macbook",
                "url": "http://100.64.1.5:2486",
                "capabilities": ["waves", "terminal"],
                "repos": [{"name": "loopflow", "wave_count": 3}],
                "connection_token": "conn-token",
                "last_heartbeat": "2026-02-26T13:00:00Z"
              }
            ]
            """
            return .json(statusCode: 200, body: body)
        }

        let service = DiscoveryService(
            authService: auth,
            baseURL: URL(string: "https://loopflow.studio")!,
            session: makeSession()
        )

        let daemons = try await service.discoverDaemons()
        XCTAssertEqual(daemons.count, 1)
        XCTAssertEqual(daemons[0].machineId, "machine-1")
        XCTAssertEqual(daemons[0].repos.first?.waveCount, 3)
    }

    func testDiscoverDaemonsRefreshesExpiringToken() async throws {
        let auth = MockAuthTokenProvider(
            token: "old-token",
            expiry: Date().addingTimeInterval(15),
            refreshedToken: "new-token"
        )
        DiscoveryStubURLProtocol.handler = { request in
            XCTAssertEqual(request.value(forHTTPHeaderField: "Authorization"), "Bearer new-token")
            return .json(statusCode: 200, body: "[]")
        }

        let service = DiscoveryService(
            authService: auth,
            baseURL: URL(string: "https://loopflow.studio")!,
            session: makeSession(),
            tokenRefreshLeadTime: 120
        )

        let daemons = try await service.discoverDaemons()
        XCTAssertTrue(daemons.isEmpty)
        XCTAssertEqual(auth.refreshCallCount, 1)
    }

    func testDiscoverDaemonsDecodesWrappedPayload() async throws {
        let auth = MockAuthTokenProvider(token: "jwt-token", expiry: Date().addingTimeInterval(3600))
        DiscoveryStubURLProtocol.handler = { _ in
            let body = """
            {
              "daemons": [
                {
                  "machine_id": "machine-2",
                  "machine_name": null,
                  "url": "https://10.0.0.9:443",
                  "capabilities": [],
                  "repos": [],
                  "connection_token": "abc",
                  "last_heartbeat": 1700000000
                }
              ]
            }
            """
            return .json(statusCode: 200, body: body)
        }

        let service = DiscoveryService(
            authService: auth,
            baseURL: URL(string: "https://loopflow.studio")!,
            session: makeSession()
        )

        let daemons = try await service.discoverDaemons()
        XCTAssertEqual(daemons.count, 1)
        XCTAssertEqual(daemons[0].id, "machine-2")
        XCTAssertNotNil(daemons[0].lastHeartbeat)
    }
}

private struct StubResponse {
    let statusCode: Int
    let body: Data
}

private extension StubResponse {
    static func json(statusCode: Int, body: String) -> Self {
        Self(statusCode: statusCode, body: Data(body.utf8))
    }
}

private func makeSession() -> URLSession {
    let config = URLSessionConfiguration.ephemeral
    config.protocolClasses = [DiscoveryStubURLProtocol.self]
    return URLSession(configuration: config)
}

private final class MockAuthTokenProvider: StudioAuthTokenProvider {
    var token: String?
    var expiry: Date?
    var refreshedToken: String
    var refreshCallCount = 0

    init(token: String?, expiry: Date?, refreshedToken: String = "refreshed-token") {
        self.token = token
        self.expiry = expiry
        self.refreshedToken = refreshedToken
    }

    func currentToken() -> String? {
        token
    }

    func tokenExpiresAt() -> Date? {
        expiry
    }

    func refreshToken() async throws -> String {
        refreshCallCount += 1
        token = refreshedToken
        return refreshedToken
    }
}

private final class DiscoveryStubURLProtocol: URLProtocol, @unchecked Sendable {
    private static let lock = NSLock()
    nonisolated(unsafe) private static var _handler: ((URLRequest) throws -> StubResponse)?

    static var handler: ((URLRequest) throws -> StubResponse)? {
        get {
            lock.lock()
            defer { lock.unlock() }
            return _handler
        }
        set {
            lock.lock()
            defer { lock.unlock() }
            _handler = newValue
        }
    }

    override class func canInit(with request: URLRequest) -> Bool {
        true
    }

    override class func canonicalRequest(for request: URLRequest) -> URLRequest {
        request
    }

    override func startLoading() {
        guard let handler = Self.handler else {
            client?.urlProtocol(self, didFailWithError: URLError(.badServerResponse))
            return
        }

        do {
            let stub = try handler(request)
            let response = HTTPURLResponse(
                url: request.url ?? URL(string: "https://loopflow.studio")!,
                statusCode: stub.statusCode,
                httpVersion: nil,
                headerFields: ["Content-Type": "application/json"]
            )!

            client?.urlProtocol(self, didReceive: response, cacheStoragePolicy: .notAllowed)
            client?.urlProtocol(self, didLoad: stub.body)
            client?.urlProtocolDidFinishLoading(self)
        } catch {
            client?.urlProtocol(self, didFailWithError: error)
        }
    }

    override func stopLoading() {}
}
