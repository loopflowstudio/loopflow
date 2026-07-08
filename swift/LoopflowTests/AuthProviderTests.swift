import Foundation
import Testing
@testable import Loopflow

@Suite("Auth provider models")
struct AuthProviderModelTests {
    @Test("list response decodes wrapped providers payload")
    func decodeProviderListResponse() throws {
        let payload = """
        {
          "providers": [
            {"provider": "github", "status": "active", "login": "octocat", "credential_type": "oauth"},
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
        #expect(decoded.providers[0].credentialType == .oauth)
        #expect(decoded.providers[1].provider == .claude)
        #expect(decoded.providers[1].status == .none)
        #expect(decoded.providers[1].credentialType == nil)
    }

    @Test("credential_type decodes apikey variant")
    func decodeApiKeyCredentialType() throws {
        let payload = """
        {
          "providers": [
            {"provider": "codex", "status": "active", "credential_type": "apikey"}
          ]
        }
        """

        let decoded = try JSONDecoder().decode(
            AuthProviderListResponse.self,
            from: Data(payload.utf8)
        )

        #expect(decoded.providers[0].credentialType == .apikey)
    }
}
