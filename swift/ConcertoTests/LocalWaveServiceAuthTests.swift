import Foundation
import Testing
@testable import LoopflowCore

@Suite("LocalWaveService auth", .serialized)
struct LocalWaveServiceAuthTests {
    @Test("listAuthProviders decodes wrapped provider list")
    func listAuthProvidersDecodesResponse() async throws {
        let service = makeService { request in
            #expect(request.httpMethod == "GET")
            #expect(request.url?.path == "/v0/auth")

            return StubResponse(
                statusCode: 200,
                body: Data(
                    """
                    {
                      "providers": [
                        {"provider": "github", "status": "active", "login": "octocat"},
                        {"provider": "claude", "status": "none", "login": null},
                        {"provider": "codex", "status": "expired", "login": "old-user"}
                      ]
                    }
                    """.utf8
                )
            )
        }

        let providers = try await service.listAuthProviders()

        #expect(providers.count == 3)
        #expect(providers[0].provider == .github)
        #expect(providers[0].status == .active)
        #expect(providers[2].status == .expired)
    }

    @Test("startAuthFlow uses long timeouts for provider launch")
    func startAuthFlowUsesLongTimeouts() async throws {
        let timeoutRecorder = TimeoutRecorder()

        let service = makeService(
            timeoutObserver: { request, resource in
                timeoutRecorder.append(request: request, resource: resource)
            },
            handler: { request in
                #expect(request.httpMethod == "POST")
                #expect(request.url?.path == "/v0/auth/github")

                return StubResponse(
                    statusCode: 200,
                    body: Data(
                        """
                        {
                          "provider": "github",
                          "verification_uri": "https://github.com/login/device",
                          "verification_uri_complete": "https://github.com/login/device?user_code=ABCD-1234",
                          "user_code": "ABCD-1234",
                          "expires_in": 900
                        }
                        """.utf8
                    )
                )
            }
        )

        let flow = try await service.startAuthFlow(provider: .github)

        #expect(flow.provider == .github)
        #expect(flow.userCode == "ABCD-1234")
        #expect(timeoutRecorder.lastRequestTimeout == 30)
        #expect(timeoutRecorder.lastResourceTimeout == 60)
    }

    @Test("disconnectProvider decodes status payload")
    func disconnectProviderDecodesStatus() async throws {
        let service = makeService { request in
            #expect(request.httpMethod == "DELETE")
            #expect(request.url?.path == "/v0/auth/claude")

            return StubResponse(
                statusCode: 200,
                body: Data(
                    """
                    {
                      "provider": "claude",
                      "status": "none",
                      "login": null
                    }
                    """.utf8
                )
            )
        }

        let result = try await service.disconnectProvider(provider: .claude)

        #expect(result.provider == .claude)
        #expect(result.status == .none)
    }
}

private struct StubResponse {
    let statusCode: Int
    let body: Data
}

// SAFETY: lock-guarded test recorder; all mutation is serialized via NSLock.
private final class TimeoutRecorder: @unchecked Sendable {
    private let lock = NSLock()
    private var requestTimeout: TimeInterval?
    private var resourceTimeout: TimeInterval?

    func append(request: TimeInterval, resource: TimeInterval) {
        lock.lock()
        defer { lock.unlock() }
        requestTimeout = request
        resourceTimeout = resource
    }

    var lastRequestTimeout: TimeInterval? {
        lock.lock()
        defer { lock.unlock() }
        return requestTimeout
    }

    var lastResourceTimeout: TimeInterval? {
        lock.lock()
        defer { lock.unlock() }
        return resourceTimeout
    }
}

private func makeService(
    timeoutObserver: @escaping @Sendable (TimeInterval, TimeInterval) -> Void = { _, _ in },
    handler: @escaping @Sendable (URLRequest) throws -> StubResponse
) -> WaveService {
    StubURLProtocol.handler = handler

    return WaveService(
        connection: .local,
        sessionFactory: { requestTimeout, resourceTimeout, delegate in
            timeoutObserver(requestTimeout, resourceTimeout)
            let config = URLSessionConfiguration.ephemeral
            config.protocolClasses = [StubURLProtocol.self]
            config.timeoutIntervalForRequest = requestTimeout
            config.timeoutIntervalForResource = resourceTimeout
            return URLSession(configuration: config, delegate: delegate, delegateQueue: nil)
        }
    )
}

// SAFETY: static handler access is synchronized with NSLock for test isolation.
private final class StubURLProtocol: URLProtocol, @unchecked Sendable {
    private static let lock = NSLock()
    nonisolated(unsafe) private static var _handler: (@Sendable (URLRequest) throws -> StubResponse)?

    static var handler: (@Sendable (URLRequest) throws -> StubResponse)? {
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
                url: request.url ?? URL(string: "http://127.0.0.1")!,
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
