// Pipeline model. Maps to LoopflowSwift/LoopflowCore/Models/Pipeline.swift

export interface StepConfig {
  model?: string
  voice?: string
  context?: string[]
}

export interface PipelineStep {
  id: string
  task?: string
  pipeline?: string  // Reference to nested pipeline
  config?: StepConfig
}

export interface PipelineDef {
  id: string
  name: string
  steps: PipelineStep[]
}

// Helper functions
export function stepConfigIsEmpty(config?: StepConfig): boolean {
  if (!config) return true
  return (
    config.model === undefined &&
    config.voice === undefined &&
    (config.context === undefined || config.context.length === 0)
  )
}

export function stepDisplayName(step: PipelineStep): string {
  if (step.task) return step.task
  if (step.pipeline) return `[${step.pipeline}]`
  return '?'
}

export function stepHasConfig(step: PipelineStep): boolean {
  return !stepConfigIsEmpty(step.config)
}
