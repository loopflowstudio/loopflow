// Step model. Maps to swift/LoopflowCore/Models/Step.swift
// A Step is the basic unit - a prompt to run with optional config.

export interface StepConfig {
  model?: string
  voice?: string
  context?: string[]
}

export interface Step {
  id: string
  prompt: string
  config?: StepConfig
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

export function stepHasConfig(step: Step): boolean {
  return !stepConfigIsEmpty(step.config)
}

// StepRun = a single execution of a step

export type StepStatus = 'running' | 'completed' | 'error'

export interface StepRun {
  id: string
  step: string  // matches Python schema
  repo: string
  worktree: string  // matches Python schema (was worktreePath)
  status: StepStatus
  startedAt: Date
  endedAt?: Date
  model: string
  runMode: string
}

export function stepRunIsCompleted(run: StepRun): boolean {
  return run.status === 'completed'
}

export function stepRunIsRunning(run: StepRun): boolean {
  return run.status === 'running'
}

export interface FileChange {
  id: string
  path: string
  kind: 'added' | 'modified' | 'deleted' | 'renamed'
  linesAdded: number
  linesRemoved: number
  diffPreview?: string
}

export interface StepResult {
  id: string
  step: string  // matches Python schema
  worktree: string
  status: StepStatus
  startedAt: Date
  endedAt: Date
  filesChanged: FileChange[]
  newCommits: string[]
  baselineSHA: string
  currentSHA: string
}

// Computed properties
export function duration(result: StepResult): number {
  return (result.endedAt.getTime() - result.startedAt.getTime()) / 1000
}

export function hasChanges(result: StepResult): boolean {
  return result.filesChanged.length > 0 || result.newCommits.length > 0
}

export function totalLinesAdded(result: StepResult): number {
  return result.filesChanged.reduce((sum, f) => sum + f.linesAdded, 0)
}

export function totalLinesRemoved(result: StepResult): number {
  return result.filesChanged.reduce((sum, f) => sum + f.linesRemoved, 0)
}
