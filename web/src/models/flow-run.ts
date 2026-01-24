// FlowRun - an execution instance of a Flow, spawned by an Agent.

export type FlowRunStatus = 'pending' | 'running' | 'completed' | 'failed' | 'cancelled'

export interface FlowRun {
  id: string
  agentId: string | null

  flow: string
  area: string
  repo: string
  goal: string[]

  status: FlowRunStatus
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

export function flowRunShortId(run: FlowRun): string {
  return run.id.slice(0, 7)
}

export function flowRunAreaDisplay(run: FlowRun): string {
  return run.area === '.' ? 'root' : run.area
}

export function flowRunGoalDisplay(run: FlowRun): string {
  return run.goal.length === 0 ? 'default' : run.goal.join(', ')
}

export function flowRunFlowDisplay(run: FlowRun): string {
  return run.flow || 'default'
}

export function flowRunStatusText(run: FlowRun): string {
  switch (run.status) {
    case 'running': return run.currentStep ?? 'Running'
    case 'pending': return 'Pending'
    case 'completed': return 'Completed'
    case 'failed': return 'Failed'
    case 'cancelled': return 'Cancelled'
  }
}
