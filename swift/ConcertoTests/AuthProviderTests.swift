import Foundation
import Testing
@testable import LoopflowCore

@Suite("Auth provider models")
struct AuthProviderModelTests {
    @Test("list response decodes wrapped providers payload")
    func decodeProviderListResponse() throws {
        let payload = """
        {
          "providers": [
            {"provider": "github", "status": "active", "login": "octocat"},
            {"provider": "claude", "status": "none", "login": null}
          ]
        }
        """

        let decoded = try JSONDecoder().decode(
            AuthProviderListResponse.self,
            from: Data(payload.utf8)
        )

        #expect(decoded.providers.count == 2)
        #expect(decoded.providers[0].provider == .github)
        #expect(decoded.providers[0].status == .active)
        #expect(decoded.providers[0].login == "octocat")
        #expect(decoded.providers[1].provider == .claude)
        #expect(decoded.providers[1].status == .none)
    }
}

@Suite("Auth event parsing")
struct AuthEventParsingTests {
    @Test("auth.flow_started parses provider and URLs")
    func parseFlowStartedEvent() throws {
        let text = """
        {
          "type": "auth.flow_started",
          "provider": "github",
          "verification_uri": "https://github.com/login/device",
          "verification_uri_complete": "https://github.com/login/device?user_code=ABCD-1234",
          "timestamp": "2026-02-26T20:44:00.000Z"
        }
        """

        let auth = try parseAuthEvent(text)

        #expect(auth.type == .flowStarted)
        #expect(auth.provider == .github)
        #expect(auth.verificationURI == "https://github.com/login/device")
        #expect(auth.verificationURIComplete == "https://github.com/login/device?user_code=ABCD-1234")
    }

    @Test("auth.connected parses login")
    func parseConnectedEvent() throws {
        let text = """
        {
          "type": "auth.connected",
          "provider": "claude",
          "login": "loopflow-user",
          "timestamp": "2026-02-26T20:45:00.000Z"
        }
        """

        let auth = try parseAuthEvent(text)

        #expect(auth.type == .connected)
        #expect(auth.provider == .claude)
        #expect(auth.login == "loopflow-user")
        #expect(auth.error == nil)
    }

    @Test("auth.failed parses nested error payload")
    func parseFailedEvent() throws {
        let text = """
        {
          "type": "auth.failed",
          "provider": "codex",
          "error": {"message": "Flow expired"},
          "timestamp": "2026-02-26T20:46:00.000Z"
        }
        """

        let auth = try parseAuthEvent(text)

        #expect(auth.type == .failed)
        #expect(auth.provider == .codex)
        #expect(auth.error == "Flow expired")
    }

    @Test("auth.disconnected parses provider")
    func parseDisconnectedEvent() throws {
        let text = """
        {
          "type": "auth.disconnected",
          "provider": "github",
          "timestamp": "2026-02-26T20:47:00.000Z"
        }
        """

        let auth = try parseAuthEvent(text)

        #expect(auth.type == .disconnected)
        #expect(auth.provider == .github)
    }
}

private enum AuthEventParseError: Error {
    case expectedAuthEvent
}

private func parseAuthEvent(_ text: String) throws -> AuthEvent {
    guard let event = EventService.parseEvent(text: text),
          case .auth(let auth) = event else {
        throw AuthEventParseError.expectedAuthEvent
    }
    return auth
}
