// Tests for ConfigLoader YAML parsing.

import Testing
@testable import Loopflow

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
        let warpConfig = LoopflowConfig(
            agentModel: nil, interactive: nil, terminal: "warp",
            ide: nil, workspace: nil, context: nil, exclude: nil,
            push: nil, pr: nil, yolo: nil
        )
        #expect(warpConfig.terminalApp == .warp)

        let itermConfig = LoopflowConfig(
            agentModel: nil, interactive: nil, terminal: "iterm",
            ide: nil, workspace: nil, context: nil, exclude: nil,
            push: nil, pr: nil, yolo: nil
        )
        #expect(itermConfig.terminalApp == .iterm)

        let defaultConfig = LoopflowConfig(
            agentModel: nil, interactive: nil, terminal: nil,
            ide: nil, workspace: nil, context: nil, exclude: nil,
            push: nil, pr: nil, yolo: nil
        )
        #expect(defaultConfig.terminalApp == .terminal)
    }

    @Test("isInteractive checks list correctly")
    func isInteractiveCheck() {
        let config = LoopflowConfig(
            agentModel: nil, interactive: ["design", "iterate"],
            terminal: nil, ide: nil, workspace: nil, context: nil,
            exclude: nil, push: nil, pr: nil, yolo: nil
        )

        #expect(config.isInteractive("design") == true)
        #expect(config.isInteractive("iterate") == true)
        #expect(config.isInteractive("implement") == false)
    }
}
