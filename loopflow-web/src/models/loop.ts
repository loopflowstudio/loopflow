// Loop model. Maps to LoopflowSwift/LoopflowCore/Models/Loop.swift

export type LoopType = 'loop' | 'flow' | 'subscribe' | 'schedule'

export type LoopStatus = 'idle' | 'running' | 'waiting' | 'error'

export type LoopMergeMode = 'pr' | 'land'

export interface Loop {
  id: string
  type: LoopType
  area: string
  goals: string[]
  flow?: string
  repo: string
  loopMain: string
  status: LoopStatus
  iteration: number
  prLimit: number
  mergeMode: LoopMergeMode
  pid?: number
  createdAt: Date
  currentRunId?: string
  currentStep?: string
  commitsAhead: number
}

export interface LoopRun {
  id: string
  loopId: string
  iteration: number
  status: LoopStatus
  startedAt: Date
  endedAt?: Date
  worktree?: string
  currentStep?: string
  error?: string
  prUrl?: string
}

// Computed properties (mirroring Swift)
export function shortId(loop: Loop): string {
  return loop.id.slice(0, 7)
}

export function areaDisplay(loop: Loop): string {
  return loop.area === '.' ? 'root' : loop.area
}

export function goalsDisplay(loop: Loop): string {
  return loop.goals.length === 0 ? 'adaptive' : loop.goals.join(', ')
}

export function flowDisplay(loop: Loop): string {
  return loop.flow ?? 'default'
}

export function statusText(loop: Loop): string {
  switch (loop.status) {
    case 'running': return loop.currentStep ?? 'Running'
    case 'waiting': return 'Waiting'
    case 'idle': return 'Idle'
    case 'error': return 'Error'
  }
}

export function iterationText(loop: Loop): string {
  return loop.iteration > 0 ? `iter ${loop.iteration}` : ''
}

// Extended interface with computed properties
declare module './loop' {
  interface Loop {
    readonly shortId: string
    readonly areaDisplay: string
    readonly goalsDisplay: string
    readonly flowDisplay: string
    readonly statusText: string
    readonly iterationText: string
  }
}
