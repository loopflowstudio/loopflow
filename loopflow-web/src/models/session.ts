// Session model. Maps to LoopflowSwift/LoopflowCore/Models/TaskSession.swift

export type SessionResultStatus = 'running' | 'completed' | 'error'

export interface TaskSession {
  id: string
  task: string
  worktreePath: string
  status: SessionResultStatus
  startedAt: Date
  endedAt?: Date
  isCompleted: boolean
}

export interface FileChange {
  id: string
  path: string
  kind: 'added' | 'modified' | 'deleted' | 'renamed'
  linesAdded: number
  linesRemoved: number
  diffPreview?: string
}

export interface SessionResult {
  id: string
  task: string
  worktree: string
  status: SessionResultStatus
  startedAt: Date
  endedAt: Date
  filesChanged: FileChange[]
  newCommits: string[]
  baselineSHA: string
  currentSHA: string
}

// Computed properties
export function duration(result: SessionResult): number {
  return (result.endedAt.getTime() - result.startedAt.getTime()) / 1000
}

export function hasChanges(result: SessionResult): boolean {
  return result.filesChanged.length > 0 || result.newCommits.length > 0
}

export function totalLinesAdded(result: SessionResult): number {
  return result.filesChanged.reduce((sum, f) => sum + f.linesAdded, 0)
}

export function totalLinesRemoved(result: SessionResult): number {
  return result.filesChanged.reduce((sum, f) => sum + f.linesRemoved, 0)
}
