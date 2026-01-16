// Service for loading and saving pipeline YAML files in .lf/pipelines/.

import Foundation

struct PipelineService {
    enum PipelineError: LocalizedError {
        case directoryNotFound
        case parseError(String)
        case writeError(String)

        var errorDescription: String? {
            switch self {
            case .directoryNotFound: return "Pipelines directory not found"
            case .parseError(let msg): return "Failed to parse pipeline: \(msg)"
            case .writeError(let msg): return "Failed to save pipeline: \(msg)"
            }
        }
    }

    /// Load all pipelines from .lf/pipelines/.
    func loadPipelines(from repoURL: URL) -> [PipelineDef] {
        let pipelinesDir = repoURL.appendingPathComponent(".lf/pipelines")

        guard FileManager.default.fileExists(atPath: pipelinesDir.path()) else {
            return []
        }

        guard let contents = try? FileManager.default.contentsOfDirectory(
            at: pipelinesDir,
            includingPropertiesForKeys: nil
        ) else {
            return []
        }

        var pipelines: [PipelineDef] = []
        for url in contents where url.pathExtension == "yaml" {
            if let pipeline = loadPipeline(from: url) {
                pipelines.append(pipeline)
            }
        }

        return pipelines.sorted { $0.name < $1.name }
    }

    /// Load a single pipeline from a YAML file.
    func loadPipeline(from url: URL) -> PipelineDef? {
        guard let contents = try? String(contentsOf: url, encoding: .utf8) else {
            return nil
        }

        let name = url.deletingPathExtension().lastPathComponent
        return parsePipelineYAML(contents, name: name)
    }

    /// Load a pipeline by name from .lf/pipelines/{name}.yaml.
    func loadPipeline(named name: String, in repoURL: URL) -> PipelineDef? {
        let pipelineURL = repoURL.appendingPathComponent(".lf/pipelines/\(name).yaml")
        return loadPipeline(from: pipelineURL)
    }

    /// Save a pipeline to .lf/pipelines/{name}.yaml.
    func savePipeline(_ pipeline: PipelineDef, in repoURL: URL) throws {
        let pipelinesDir = repoURL.appendingPathComponent(".lf/pipelines")

        // Create directory if needed
        try FileManager.default.createDirectory(at: pipelinesDir, withIntermediateDirectories: true)

        let pipelineURL = pipelinesDir.appendingPathComponent("\(pipeline.name).yaml")
        let yaml = serializePipelineYAML(pipeline)

        do {
            try yaml.write(to: pipelineURL, atomically: true, encoding: .utf8)
        } catch {
            throw PipelineError.writeError(error.localizedDescription)
        }
    }

    /// Delete a pipeline file.
    func deletePipeline(named name: String, in repoURL: URL) throws {
        let pipelineURL = repoURL.appendingPathComponent(".lf/pipelines/\(name).yaml")
        try FileManager.default.removeItem(at: pipelineURL)
    }

    // MARK: - YAML Parsing

    func parsePipelineYAML(_ contents: String, name: String) -> PipelineDef? {
        var steps: [PipelineStep] = []
        var inSteps = false
        var currentStep: (task: String?, pipeline: String?, config: StepConfig?) = (nil, nil, nil)
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
                if let task = currentStep.task {
                    var step = PipelineStep(task: task, pipeline: currentStep.pipeline)
                    if inConfig {
                        var config = currentStep.config ?? StepConfig()
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
                currentStep = (nil, nil, nil)
                inConfig = false
                configContext = []
                inContext = false

                let value = String(trimmed.dropFirst(2)).trimmingCharacters(in: .whitespaces)

                // Simple string task: "- design"
                if !value.contains(":") {
                    currentStep.task = value.trimmingCharacters(in: CharacterSet(charactersIn: "\"'"))
                    continue
                }

                // Task with key: "- task: design"
                if let colonIdx = value.firstIndex(of: ":") {
                    let key = String(value[..<colonIdx]).trimmingCharacters(in: .whitespaces)
                    let val = String(value[value.index(after: colonIdx)...])
                        .trimmingCharacters(in: .whitespaces)
                        .trimmingCharacters(in: CharacterSet(charactersIn: "\"'"))

                    if key == "task" {
                        currentStep.task = val
                    } else if key == "pipeline" {
                        currentStep.pipeline = val
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

                if key == "task" {
                    currentStep.task = value
                    inContext = false
                } else if key == "pipeline" {
                    currentStep.pipeline = value
                    inContext = false
                } else if key == "config" {
                    inConfig = true
                    currentStep.config = StepConfig()
                    inContext = false
                } else if inConfig {
                    if key == "model" {
                        currentStep.config?.model = value
                        inContext = false
                    } else if key == "voice" {
                        currentStep.config?.voice = value
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
        if let task = currentStep.task {
            var step = PipelineStep(task: task, pipeline: currentStep.pipeline)
            if inConfig {
                var config = currentStep.config ?? StepConfig()
                if !configContext.isEmpty {
                    config.context = configContext
                }
                if !config.isEmpty {
                    step.config = config
                }
            }
            steps.append(step)
        } else if let pipelineRef = currentStep.pipeline {
            steps.append(PipelineStep(pipeline: pipelineRef))
        }

        return PipelineDef(name: name, steps: steps)
    }

    func serializePipelineYAML(_ pipeline: PipelineDef) -> String {
        var lines: [String] = ["steps:"]

        for step in pipeline.steps {
            if let task = step.task {
                if step.hasConfig {
                    lines.append("  - task: \(task)")
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
                    lines.append("  - \(task)")
                }
            } else if let pipelineRef = step.pipeline {
                lines.append("  - pipeline: \(pipelineRef)")
            }
        }

        return lines.joined(separator: "\n") + "\n"
    }
}
