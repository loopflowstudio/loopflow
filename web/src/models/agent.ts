// Agent - an autonomous AI coding agent.
// Activation modes: loop (continuous), watch (on file change), cron (scheduled).

export type AgentStatus = 'idle' | 'running' | 'waiting' | 'error'

export type MergeMode = 'pr' | 'land'

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
