// Models for lfd daemon. Maps to swift/LoopflowCore/Models/Job.swift

// MARK: - FlowRun (execution instance of a Flow)

export type FlowRunStatus = 'pending' | 'running' | 'completed' | 'failed' | 'cancelled'

export interface FlowRun {
  id: string
  agentId: string | null

  flow: string
  area: string
  repo: string
  voice: string[]

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

export function flowRunVoiceDisplay(run: FlowRun): string {
  return run.voice.length === 0 ? 'default' : run.voice.join(', ')
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

// MARK: - Agent status

export type AgentStatus = 'idle' | 'running' | 'waiting' | 'error'

export type MergeMode = 'pr' | 'land'

// MARK: - Agent (unified model for loop/watch/cron)

export interface Agent {
  id: string
  flow: string
  voice: string[]
  area: string[]
  repo: string

  status: AgentStatus
  iteration: number

  mainBranch: string
  prLimit: number
  mergeMode: MergeMode

  pid?: number
  createdAt: Date

  // Activation config (determines mode)
  watchPaths?: string
  cron?: string
  lastMainSha?: string
}

export function agentShortId(agent: Agent): string {
  return agent.id.slice(0, 7)
}

export function agentMode(agent: Agent): 'watch' | 'cron' | 'loop' {
  if (agent.watchPaths) return 'watch'
  if (agent.cron) return 'cron'
  return 'loop'
}

export function agentAreaDisplay(agent: Agent): string {
  return agent.area[0] === '.' ? 'root' : agent.area.join(', ')
}

export function agentVoiceDisplay(agent: Agent): string {
  return agent.voice.length === 0 ? 'default' : agent.voice.join(', ')
}

export function agentFlowDisplay(agent: Agent): string {
  return agent.flow || 'default'
}

export function agentStatusText(agent: Agent): string {
  switch (agent.status) {
    case 'running': return 'Running'
    case 'waiting': return 'Waiting'
    case 'idle': return 'Idle'
    case 'error': return 'Error'
  }
}

export function agentIterationText(agent: Agent): string {
  return agent.iteration > 0 ? `iter ${agent.iteration}` : ''
}
