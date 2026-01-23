// Models for lfd daemon. Maps to swift/LoopflowCore/Models/Job.swift

// MARK: - Run (execution instance)

export type RunStatus = 'pending' | 'running' | 'completed' | 'failed' | 'cancelled'

export interface Run {
  id: string
  parent: string | null  // "loop:<id>" | "subscription:<id>" | "schedule:<id>" | null

  flow: string
  area: string
  repo: string
  goals: string[]

  status: RunStatus
  iteration: number

  worktree?: string
  branch?: string
  currentStep?: string
  error?: string
  prUrl?: string

  startedAt?: Date
  endedAt?: Date
  createdAt: Date
}

export function runShortId(run: Run): string {
  return run.id.slice(0, 7)
}

export function runParentType(run: Run): string | null {
  if (!run.parent) return null
  return run.parent.split(':')[0]
}

export function runParentId(run: Run): string | null {
  if (!run.parent) return null
  const parts = run.parent.split(':')
  return parts.length > 1 ? parts.slice(1).join(':') : null
}

export function runAreaDisplay(run: Run): string {
  return run.area === '.' ? 'root' : run.area
}

export function runGoalsDisplay(run: Run): string {
  return run.goals.length === 0 ? 'adaptive' : run.goals.join(', ')
}

export function runFlowDisplay(run: Run): string {
  return run.flow || 'default'
}

export function runStatusText(run: Run): string {
  switch (run.status) {
    case 'running': return run.currentStep ?? 'Running'
    case 'pending': return 'Pending'
    case 'completed': return 'Completed'
    case 'failed': return 'Failed'
    case 'cancelled': return 'Cancelled'
  }
}

// MARK: - Trigger status (shared by Loop, Subscription, Schedule)

export type TriggerStatus = 'idle' | 'running' | 'waiting' | 'error'

export type MergeMode = 'pr' | 'land'

// MARK: - Loop (continuous runner)

export interface Loop {
  id: string
  flow: string
  area: string
  repo: string
  goals: string[]

  status: TriggerStatus
  iteration: number

  mainBranch: string
  prLimit: number
  mergeMode: MergeMode

  pid?: number
  createdAt: Date

  // Runtime state
  currentRunId?: string
  currentStep?: string
  commitsAhead: number
}

export function loopShortId(loop: Loop): string {
  return loop.id.slice(0, 7)
}

export function loopAreaDisplay(loop: Loop): string {
  return loop.area === '.' ? 'root' : loop.area
}

export function loopGoalsDisplay(loop: Loop): string {
  return loop.goals.length === 0 ? 'adaptive' : loop.goals.join(', ')
}

export function loopFlowDisplay(loop: Loop): string {
  return loop.flow || 'default'
}

export function loopStatusText(loop: Loop): string {
  switch (loop.status) {
    case 'running': return loop.currentStep ?? 'Running'
    case 'waiting': return 'Waiting'
    case 'idle': return 'Idle'
    case 'error': return 'Error'
  }
}

export function loopIterationText(loop: Loop): string {
  return loop.iteration > 0 ? `iter ${loop.iteration}` : ''
}

// MARK: - Subscription (pathset watcher)

export interface Subscription {
  id: string
  flow: string
  area: string
  repo: string
  goals: string[]

  pathset: string
  lastMainSha?: string

  status: TriggerStatus
  iteration: number

  mainBranch: string
  prLimit: number
  mergeMode: MergeMode

  pid?: number
  createdAt: Date

  // Runtime state
  currentRunId?: string
  currentStep?: string
  commitsAhead: number
}

export function subscriptionShortId(sub: Subscription): string {
  return sub.id.slice(0, 7)
}

export function subscriptionAreaDisplay(sub: Subscription): string {
  return sub.area === '.' ? 'root' : sub.area
}

export function subscriptionGoalsDisplay(sub: Subscription): string {
  return sub.goals.length === 0 ? 'adaptive' : sub.goals.join(', ')
}

export function subscriptionFlowDisplay(sub: Subscription): string {
  return sub.flow || 'default'
}

export function subscriptionStatusText(sub: Subscription): string {
  switch (sub.status) {
    case 'running': return sub.currentStep ?? 'Running'
    case 'waiting': return 'Waiting'
    case 'idle': return 'Idle'
    case 'error': return 'Error'
  }
}

export function subscriptionIterationText(sub: Subscription): string {
  return sub.iteration > 0 ? `iter ${sub.iteration}` : ''
}

// MARK: - Schedule (cron trigger)

export interface Schedule {
  id: string
  flow: string
  area: string
  repo: string
  goals: string[]

  cron: string

  status: TriggerStatus
  iteration: number

  mainBranch: string
  prLimit: number
  mergeMode: MergeMode

  pid?: number
  createdAt: Date

  // Runtime state
  currentRunId?: string
  currentStep?: string
  commitsAhead: number
}

export function scheduleShortId(sched: Schedule): string {
  return sched.id.slice(0, 7)
}

export function scheduleAreaDisplay(sched: Schedule): string {
  return sched.area === '.' ? 'root' : sched.area
}

export function scheduleGoalsDisplay(sched: Schedule): string {
  return sched.goals.length === 0 ? 'adaptive' : sched.goals.join(', ')
}

export function scheduleFlowDisplay(sched: Schedule): string {
  return sched.flow || 'default'
}

export function scheduleStatusText(sched: Schedule): string {
  switch (sched.status) {
    case 'running': return sched.currentStep ?? 'Running'
    case 'waiting': return 'Waiting'
    case 'idle': return 'Idle'
    case 'error': return 'Error'
  }
}

export function scheduleIterationText(sched: Schedule): string {
  return sched.iteration > 0 ? `iter ${sched.iteration}` : ''
}

// MARK: - Trigger (any trigger type)

export type Trigger =
  | { type: 'loop'; value: Loop }
  | { type: 'subscription'; value: Subscription }
  | { type: 'schedule'; value: Schedule }

export function triggerId(trigger: Trigger): string {
  return trigger.value.id
}

export function triggerStatus(trigger: Trigger): TriggerStatus {
  return trigger.value.status
}

export function triggerArea(trigger: Trigger): string {
  return trigger.value.area
}

export function triggerRepo(trigger: Trigger): string {
  return trigger.value.repo
}

export function triggerMainBranch(trigger: Trigger): string {
  return trigger.value.mainBranch
}

export function triggerCommitsAhead(trigger: Trigger): number {
  return trigger.value.commitsAhead
}

export function triggerAreaDisplay(trigger: Trigger): string {
  return trigger.value.area === '.' ? 'root' : trigger.value.area
}
