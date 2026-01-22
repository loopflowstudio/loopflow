// Tests for ConfigLoader YAML parsing.

import Testing
@testable import LoopflowCore

@Suite("Config Loader")
struct ConfigLoaderTests {

    @Test("Parses basic config YAML")
    func parseBasicConfig() {
        let loader = ConfigLoader()
        let yaml = """
        agent_model: claude:opus
        terminal: warp
        ide: cursor
        push: true
        pr: false
        """

        let config = loader.parseYAML(yaml)

        #expect(config?.agentModel == "claude:opus")
        #expect(config?.terminal == "warp")
        #expect(config?.ide == "cursor")
        #expect(config?.push == true)
        #expect(config?.pr == false)
    }

    @Test("Parses interactive list")
    func parseInteractiveList() {
        let loader = ConfigLoader()
        let yaml = """
        interactive:
          - design
          - iterate
        """

        let config = loader.parseYAML(yaml)

        #expect(config?.interactive?.count == 2)
        #expect(config?.interactive?.contains("design") == true)
        #expect(config?.interactive?.contains("iterate") == true)
    }

    @Test("Returns correct terminal app")
    func terminalAppMapping() {
        let loader = ConfigLoader()

        let warpConfig = loader.parseYAML("terminal: warp")
        #expect(warpConfig?.terminalApp == TerminalApp.warp)

        let itermConfig = loader.parseYAML("terminal: iterm")
        #expect(itermConfig?.terminalApp == TerminalApp.iterm)

        let defaultConfig = loader.parseYAML("")
        #expect(defaultConfig?.terminalApp == TerminalApp.warp)
    }

    @Test("isInteractive checks list correctly")
    func isInteractiveCheck() {
        let loader = ConfigLoader()
        let yaml = """
        interactive:
          - design
          - iterate
        """

        let config = loader.parseYAML(yaml)

        #expect(config?.isInteractive("design") == true)
        #expect(config?.isInteractive("iterate") == true)
        #expect(config?.isInteractive("implement") == false)
    }
}
