// Agent - an autonomous AI coding agent.
// Activation modes: loop (continuous), watch (on file change), cron (scheduled).

export type AgentMode = 'loop' | 'watch' | 'cron'

export type AgentStatus = 'idle' | 'running' | 'waiting' | 'error'

export type MergeMode = 'pr' | 'land'

export interface Agent {
  id: string
  flow: string
  goal: string[]
  area: string[]
  repo: string

  mode: AgentMode
  status: AgentStatus
  iteration: number

  mainBranch: string
  prLimit: number
  mergeMode: MergeMode

  pid?: number
  createdAt: Date

  // Trigger config (for watch/cron modes)
  watchPaths?: string
  cron?: string
  lastMainSha?: string
}

export function agentShortId(agent: Agent): string {
  return agent.id.slice(0, 7)
}

export function agentAreaDisplay(agent: Agent): string {
  return agent.area[0] === '.' ? 'root' : agent.area.join(', ')
}

export function agentGoalDisplay(agent: Agent): string {
  return agent.goal.length === 0 ? 'default' : agent.goal.join(', ')
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
