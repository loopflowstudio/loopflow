// Service for loading and saving flow YAML files in .lf/flows/.

import Foundation

public struct FlowService: @unchecked Sendable {
    public init() {}

    enum FlowError: LocalizedError {
        case directoryNotFound
        case parseError(String)
        case writeError(String)

        var errorDescription: String? {
            switch self {
            case .directoryNotFound: return "Flows directory not found"
            case .parseError(let msg): return "Failed to parse flow: \(msg)"
            case .writeError(let msg): return "Failed to save flow: \(msg)"
            }
        }
    }

    /// Load all flows from .lf/flows/.
    public func loadFlows(from repoURL: URL) -> [FlowDef] {
        let flowsDir = repoURL.appendingPathComponent(".lf/flows")

        guard FileManager.default.fileExists(atPath: flowsDir.path()) else {
            return []
        }

        guard let contents = try? FileManager.default.contentsOfDirectory(
            at: flowsDir,
            includingPropertiesForKeys: nil
        ) else {
            return []
        }

        var flows: [FlowDef] = []
        for url in contents where url.pathExtension == "yaml" {
            if let flow = loadFlow(from: url) {
                flows.append(flow)
            }
        }

        return flows.sorted { $0.name < $1.name }
    }

    /// Load a single flow from a YAML file.
    public func loadFlow(from url: URL) -> FlowDef? {
        guard let contents = try? String(contentsOf: url, encoding: .utf8) else {
            return nil
        }

        let name = url.deletingPathExtension().lastPathComponent
        return parseFlowYAML(contents, name: name)
    }

    /// Load a flow by name from .lf/flows/{name}.yaml.
    public func loadFlow(named name: String, in repoURL: URL) -> FlowDef? {
        let flowURL = repoURL.appendingPathComponent(".lf/flows/\(name).yaml")
        return loadFlow(from: flowURL)
    }

    /// Save a flow to .lf/flows/{name}.yaml.
    public func saveFlow(_ flow: FlowDef, in repoURL: URL) throws {
        let flowsDir = repoURL.appendingPathComponent(".lf/flows")

        // Create directory if needed
        try FileManager.default.createDirectory(at: flowsDir, withIntermediateDirectories: true)

        let flowURL = flowsDir.appendingPathComponent("\(flow.name).yaml")
        let yaml = serializeFlowYAML(flow)

        do {
            try yaml.write(to: flowURL, atomically: true, encoding: .utf8)
        } catch {
            throw FlowError.writeError(error.localizedDescription)
        }
    }

    /// Delete a flow file.
    public func deleteFlow(named name: String, in repoURL: URL) throws {
        let flowURL = repoURL.appendingPathComponent(".lf/flows/\(name).yaml")
        try FileManager.default.removeItem(at: flowURL)
    }

    // MARK: - YAML Parsing

    public func parseFlowYAML(_ contents: String, name: String) -> FlowDef? {
        var steps: [Step] = []
        var inSteps = false
        var currentPrompt: String?
        var currentConfig: StepConfig?
        var inConfig = false
        var configContext: [String] = []
        var inContext = false

        let lines = contents.components(separatedBy: .newlines)

        for line in lines {
            let trimmed = line.trimmingCharacters(in: .whitespaces)

            // Skip comments and empty lines
            if trimmed.isEmpty || trimmed.hasPrefix("#") {
                continue
            }

            // Check for steps: section
            if trimmed == "steps:" {
                inSteps = true
                continue
            }

            guard inSteps else { continue }

            // List item (new step)
            if trimmed.hasPrefix("- ") {
                // Save previous step if exists
                if let prompt = currentPrompt {
                    var step = Step(prompt: prompt)
                    if inConfig {
                        var config = currentConfig ?? StepConfig()
                        if !configContext.isEmpty {
                            config.context = configContext
                        }
                        if !config.isEmpty {
                            step.config = config
                        }
                    }
                    steps.append(step)
                }

                // Reset for new step
                currentPrompt = nil
                currentConfig = nil
                inConfig = false
                configContext = []
                inContext = false

                let value = String(trimmed.dropFirst(2)).trimmingCharacters(in: .whitespaces)

                // Simple string step: "- design"
                if !value.contains(":") {
                    currentPrompt = value.trimmingCharacters(in: CharacterSet(charactersIn: "\"'"))
                    continue
                }

                // Step with key: "- prompt: design"
                if let colonIdx = value.firstIndex(of: ":") {
                    let key = String(value[..<colonIdx]).trimmingCharacters(in: .whitespaces)
                    let val = String(value[value.index(after: colonIdx)...])
                        .trimmingCharacters(in: .whitespaces)
                        .trimmingCharacters(in: CharacterSet(charactersIn: "\"'"))

                    if key == "prompt" {
                        currentPrompt = val
                    }
                }
                continue
            }

            // Context list items
            if inContext && trimmed.hasPrefix("- ") {
                let value = String(trimmed.dropFirst(2))
                    .trimmingCharacters(in: .whitespaces)
                    .trimmingCharacters(in: CharacterSet(charactersIn: "\"'"))
                configContext.append(value)
                continue
            }

            // Nested key:value
            if let colonIdx = trimmed.firstIndex(of: ":") {
                let key = String(trimmed[..<colonIdx]).trimmingCharacters(in: .whitespaces)
                let value = String(trimmed[trimmed.index(after: colonIdx)...])
                    .trimmingCharacters(in: .whitespaces)
                    .trimmingCharacters(in: CharacterSet(charactersIn: "\"'"))

                if key == "prompt" {
                    currentPrompt = value
                    inContext = false
                } else if key == "config" {
                    inConfig = true
                    currentConfig = StepConfig()
                    inContext = false
                } else if inConfig {
                    if key == "model" {
                        currentConfig?.model = value
                        inContext = false
                    } else if key == "voice" {
                        currentConfig?.voice = value
                        inContext = false
                    } else if key == "context" {
                        inContext = true
                        if !value.isEmpty {
                            // Single-line context value
                            configContext = [value]
                            inContext = false
                        }
                    }
                }
            }
        }

        // Save last step
        if let prompt = currentPrompt {
            var step = Step(prompt: prompt)
            if inConfig {
                var config = currentConfig ?? StepConfig()
                if !configContext.isEmpty {
                    config.context = configContext
                }
                if !config.isEmpty {
                    step.config = config
                }
            }
            steps.append(step)
        }

        return FlowDef(name: name, steps: steps)
    }

    public func serializeFlowYAML(_ flow: FlowDef) -> String {
        var lines: [String] = ["steps:"]

        for step in flow.steps {
            if step.hasConfig {
                lines.append("  - prompt: \(step.prompt)")
                lines.append("    config:")
                if let model = step.config?.model {
                    lines.append("      model: \(model)")
                }
                if let voice = step.config?.voice {
                    lines.append("      voice: \(voice)")
                }
                if let context = step.config?.context, !context.isEmpty {
                    lines.append("      context:")
                    for path in context {
                        lines.append("        - \(path)")
                    }
                }
            } else {
                lines.append("  - \(step.prompt)")
            }
        }

        return lines.joined(separator: "\n") + "\n"
    }
}
