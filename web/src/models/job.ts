// Job model. Maps to swift/LoopflowCore/Models/Job.swift

// Execution mode: how many times the agent runs
export type ExecutionMode = 'continuous' | 'one_shot'

// Trigger type: what causes the agent to run
export type TriggerType = 'manual' | 'pathset' | 'cron'

// Legacy type enum - combines execution mode and trigger
// Kept for backwards compatibility with database
export type JobType = 'loop' | 'flow' | 'subscribe' | 'schedule'

export type JobStatus = 'idle' | 'running' | 'waiting' | 'error'

export type JobMergeMode = 'pr' | 'land'

export interface Job {
  id: string
  type: JobType
  area: string
  goals: string[]
  flow?: string
  repo: string
  jobMain: string
  status: JobStatus
  iteration: number
  prLimit: number
  mergeMode: JobMergeMode
  pid?: number
  createdAt: Date
  currentRunId?: string
  currentStep?: string
  commitsAhead: number
}

export interface JobRun {
  id: string
  jobId: string
  iteration: number
  status: JobStatus
  startedAt: Date
  endedAt?: Date
  worktree?: string
  currentStep?: string
  error?: string
  prUrl?: string
}

// Computed properties (mirroring Swift)
export function shortId(job: Job): string {
  return job.id.slice(0, 7)
}

export function areaDisplay(job: Job): string {
  return job.area === '.' ? 'root' : job.area
}

export function goalsDisplay(job: Job): string {
  return job.goals.length === 0 ? 'adaptive' : job.goals.join(', ')
}

export function flowDisplay(job: Job): string {
  return job.flow ?? 'default'
}

export function statusText(job: Job): string {
  switch (job.status) {
    case 'running': return job.currentStep ?? 'Running'
    case 'waiting': return 'Waiting'
    case 'idle': return 'Idle'
    case 'error': return 'Error'
  }
}

export function iterationText(job: Job): string {
  return job.iteration > 0 ? `iter ${job.iteration}` : ''
}

// Semantic accessors (derived from type)
export function executionMode(job: Job): ExecutionMode {
  return job.type === 'flow' ? 'one_shot' : 'continuous'
}

export function triggerType(job: Job): TriggerType {
  switch (job.type) {
    case 'loop':
    case 'flow':
      return 'manual'
    case 'subscribe':
      return 'pathset'
    case 'schedule':
      return 'cron'
  }
}

export function isOneShot(job: Job): boolean {
  return executionMode(job) === 'one_shot'
}

export function isTriggered(job: Job): boolean {
  return triggerType(job) !== 'manual'
}

// Backwards compatibility aliases
export type LoopType = JobType
export type LoopStatus = JobStatus
export type LoopMergeMode = JobMergeMode
export type Loop = Job
export type LoopRun = JobRun
