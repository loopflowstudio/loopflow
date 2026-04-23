import Foundation
import Testing
@testable import LoopflowCore

@Suite("Catalog")
struct CatalogTests {
    @Test("catalog response round-trips nested flow items")
    func catalogResponseRoundTripsNestedFlowItems() throws {
        let response = try JSONDecoder().decode(
            CatalogResponse.self,
            from: Data(sampleCatalogResponse.utf8)
        )

        let encoded = try JSONEncoder().encode(response)
        let roundTripped = try JSONDecoder().decode(CatalogResponse.self, from: encoded)

        #expect(roundTripped.result == response.result)
        #expect(roundTripped.result.flowsByName["build"]?.category == "Build")
        #expect(roundTripped.result.stepsByName["gate"]?.source == .repo)
    }

    @Test("fetchCatalog sends repo query and computes direct parents")
    func fetchCatalogSendsRepoQueryAndComputesDirectParents() async throws {
        let service = makeCatalogService { request in
            #expect(request.httpMethod == "GET")
            let url = try #require(request.url)
            let components = try #require(URLComponents(url: url, resolvingAgainstBaseURL: false))
            #expect(components.path == "/v0/catalog")
            #expect(components.queryItems?.first(where: { $0.name == "repo" })?.value == "/tmp/repo")

            return CatalogStubResponse(
                statusCode: 200,
                body: Data(sampleCatalogResponse.utf8)
            )
        }

        let catalog = try await service.fetchCatalog(repo: "/tmp/repo")

        #expect(catalog.directParents(of: "gate").map(\.name) == ["build", "code"])
        #expect(catalog.directParents(of: "code").map(\.name) == ["build"])
    }
}

private let sampleCatalogResponse = """
{
  "ok": true,
  "result": {
    "flows": [
      {
        "name": "build",
        "category": "Build",
        "source": "builtin",
        "items": [
          {"type": "FlowRef", "data": "code"},
          {
            "type": "Loop",
            "data": {
              "steps": [
                {"type": "Step", "data": {"name": "implement", "interactive": false}}
              ],
              "exit": {
                "router": "gate",
                "paths": {
                  "done": {"description": "Ship it"}
                }
              }
            }
          }
        ]
      },
      {
        "name": "code",
        "category": "Build",
        "source": "builtin",
        "items": [
          {"type": "Step", "data": {"name": "implement", "interactive": false}},
          {"type": "Step", "data": {"name": "gate", "interactive": false}}
        ]
      }
    ],
    "steps": [
      {
        "name": "implement",
        "category": "Build",
        "source": "builtin",
        "description": "Build from a design doc",
        "interactive": false
      },
      {
        "name": "gate",
        "category": "Build",
        "source": "repo",
        "description": "Ship-ready code and reviewer-friendly docs",
        "interactive": false
      }
    ]
  }
}
"""

private struct CatalogStubResponse {
    let statusCode: Int
    let body: Data
}

private func makeCatalogService(
    handler: @escaping @Sendable (URLRequest) throws -> CatalogStubResponse
) -> WaveService {
    CatalogStubURLProtocol.handler = handler

    return WaveService(
        connection: .local,
        sessionFactory: { requestTimeout, resourceTimeout, delegate in
            let config = URLSessionConfiguration.ephemeral
            config.protocolClasses = [CatalogStubURLProtocol.self]
            config.timeoutIntervalForRequest = requestTimeout
            config.timeoutIntervalForResource = resourceTimeout
            return URLSession(configuration: config, delegate: delegate, delegateQueue: nil)
        }
    )
}

// SAFETY: static handler access is synchronized with NSLock for test isolation.
private final class CatalogStubURLProtocol: URLProtocol, @unchecked Sendable {
    private static let lock = NSLock()
    nonisolated(unsafe) private static var _handler: (@Sendable (URLRequest) throws -> CatalogStubResponse)?

    static var handler: (@Sendable (URLRequest) throws -> CatalogStubResponse)? {
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
