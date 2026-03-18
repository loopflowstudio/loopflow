import Foundation
import Testing
@testable import LoopflowCore

@Suite("SecretsProviderStatus decoding")
struct SecretsProviderStatusTests {
    @Test("connected status decodes with keys")
    func decodeConnectedStatus() throws {
        let payload = """
        {
          "provider": "doppler",
          "connected": true,
          "project": "myapp",
          "config": "production",
          "keys": [
            {"env_name": "ANTHROPIC_API_KEY", "provider": "claude", "present": true},
            {"env_name": "OPENAI_API_KEY", "provider": "codex", "present": false}
          ]
        }
        """

        let status = try JSONDecoder().decode(
            SecretsProviderStatus.self,
            from: Data(payload.utf8)
        )

        #expect(status.provider == "doppler")
        #expect(status.connected == true)
        #expect(status.project == "myapp")
        #expect(status.config == "production")
        #expect(status.keys.count == 2)
        #expect(status.presentKeys.count == 1)
        #expect(status.missingKeys.count == 1)
        #expect(status.presentKeys[0].envName == "ANTHROPIC_API_KEY")
        #expect(status.missingKeys[0].envName == "OPENAI_API_KEY")
    }

    @Test("disconnected status decodes with null optionals")
    func decodeDisconnectedStatus() throws {
        let payload = """
        {
          "provider": "",
          "connected": false,
          "project": null,
          "config": null,
          "keys": [
            {"env_name": "ANTHROPIC_API_KEY", "provider": "claude", "present": false},
            {"env_name": "OPENAI_API_KEY", "provider": "codex", "present": false}
          ]
        }
        """

        let status = try JSONDecoder().decode(
            SecretsProviderStatus.self,
            from: Data(payload.utf8)
        )

        #expect(status.connected == false)
        #expect(status.project == nil)
        #expect(status.config == nil)
        #expect(status.presentKeys.isEmpty)
        #expect(status.missingKeys.count == 2)
    }

    @Test("static disconnected has default key mappings")
    func disconnectedDefault() {
        let status = SecretsProviderStatus.disconnected

        #expect(status.connected == false)
        #expect(status.keys.count == 2)
        #expect(status.keys.contains { $0.envName == "ANTHROPIC_API_KEY" })
        #expect(status.keys.contains { $0.envName == "OPENAI_API_KEY" })
        #expect(status.presentKeys.isEmpty)
    }
}

@Suite("DopplerProject decoding")
struct DopplerProjectTests {
    @Test("project decodes from API response")
    func decodeProject() throws {
        let payload = """
        {"slug": "my-app", "name": "My App"}
        """
        let project = try JSONDecoder().decode(DopplerProject.self, from: Data(payload.utf8))
        #expect(project.slug == "my-app")
        #expect(project.name == "My App")
        #expect(project.id == "my-app")
    }
}

@Suite("DopplerConfig decoding")
struct DopplerConfigTests {
    @Test("config decodes from API response")
    func decodeConfig() throws {
        let payload = """
        {"name": "dev", "environment": "development"}
        """
        let config = try JSONDecoder().decode(DopplerConfig.self, from: Data(payload.utf8))
        #expect(config.name == "dev")
        #expect(config.environment == "development")
        #expect(config.id == "dev")
    }
}

@Suite("Smart default config selection")
struct SmartDefaultConfigTests {
    @Test("prefers dev over others")
    func prefersDev() {
        let configs = [
            DopplerConfig(name: "prod", environment: "production"),
            DopplerConfig(name: "dev", environment: "development"),
            DopplerConfig(name: "staging", environment: "staging"),
        ]
        #expect(smartDefaultConfig(configs)?.name == "dev")
    }

    @Test("falls back to prod when no dev")
    func fallsBackToProd() {
        let configs = [
            DopplerConfig(name: "prod", environment: "production"),
            DopplerConfig(name: "staging", environment: "staging"),
        ]
        #expect(smartDefaultConfig(configs)?.name == "prod")
    }

    @Test("falls back to prd")
    func fallsBackToPrd() {
        let configs = [
            DopplerConfig(name: "prd", environment: "production"),
            DopplerConfig(name: "staging", environment: "staging"),
        ]
        #expect(smartDefaultConfig(configs)?.name == "prd")
    }

    @Test("falls back to first when no preferred match")
    func fallsBackToFirst() {
        let configs = [
            DopplerConfig(name: "custom", environment: "custom"),
        ]
        #expect(smartDefaultConfig(configs)?.name == "custom")
    }

    @Test("returns nil for empty list")
    func emptyReturnsNil() {
        let configs: [DopplerConfig] = []
        #expect(smartDefaultConfig(configs) == nil)
    }
}

@Suite("Secrets event parsing")
struct SecretsEventParsingTests {
    @Test("secrets.connected parses as secrets event")
    func parseConnectedEvent() throws {
        let text = """
        {
          "type": "secrets.connected",
          "provider": "doppler",
          "timestamp": "2026-03-17T15:00:00.000Z"
        }
        """

        guard let event = EventService.parseEvent(text: text),
              case .secrets(let secretsEvent) = event else {
            throw SecretsParseError.expectedSecretsEvent
        }

        #expect(secretsEvent.type == .connected)
        #expect(secretsEvent.provider == "doppler")
    }

    @Test("secrets.synced parses as secrets event")
    func parseSyncedEvent() throws {
        let text = """
        {
          "type": "secrets.synced",
          "provider": "doppler",
          "timestamp": "2026-03-17T15:01:00.000Z"
        }
        """

        guard let event = EventService.parseEvent(text: text),
              case .secrets(let secretsEvent) = event else {
            throw SecretsParseError.expectedSecretsEvent
        }

        #expect(secretsEvent.type == .synced)
    }

    @Test("secrets.disconnected parses as secrets event")
    func parseDisconnectedEvent() throws {
        let text = """
        {
          "type": "secrets.disconnected",
          "timestamp": "2026-03-17T15:02:00.000Z"
        }
        """

        guard let event = EventService.parseEvent(text: text),
              case .secrets(let secretsEvent) = event else {
            throw SecretsParseError.expectedSecretsEvent
        }

        #expect(secretsEvent.type == .disconnected)
        #expect(secretsEvent.provider == nil)
    }
}

private enum SecretsParseError: Error {
    case expectedSecretsEvent
}
