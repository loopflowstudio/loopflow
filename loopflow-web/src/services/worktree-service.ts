// Worktree service. Maps to LoopflowSwift/LoopflowCore/Services/WorktreeService.swift

import type { Worktree } from '@/models/worktree'
import { shortName, commitsText } from '@/models/worktree'
import { LfdClient } from './lfd-client'

export class WorktreeService {
  private client: LfdClient

  constructor(repoPath: string) {
    this.client = new LfdClient(repoPath)
  }

  async list(): Promise<Worktree[]> {
    const data = await this.client.get<WorktreeJSON[]>('/worktrees')

    return data.map((json): Worktree => ({
      path: json.path,
      branch: json.branch,
      baseBranch: json.base_branch,
      isDirty: (json.working_tree?.staged ?? false) ||
               (json.working_tree?.modified ?? false) ||
               (json.working_tree?.untracked ?? false),
      aheadMain: json.main?.ahead ?? 0,
      behindMain: json.main?.behind ?? 0,
      aheadRemote: json.remote?.ahead ?? 0,
      behindRemote: json.remote?.behind ?? 0,
      prURL: json.ci?.source === 'pr' ? json.ci.url : undefined,
      prNumber: extractPRNumber(json.ci?.url),
      prState: json.ci?.state as Worktree['prState'],
      hasCodeWorkspace: false,  // Not in JSON, computed locally
      isRebasing: json.operation_state === 'rebase',
      isMerging: json.operation_state === 'merge',
      staleness: json.prunable ? { kind: 'merged' } : { kind: 'active' },
      ciStatus: undefined,
      // Add computed properties
      get shortName() { return shortName(this) },
      get commitsText() { return commitsText(this) },
    }))
  }
}

function extractPRNumber(url?: string): number | undefined {
  if (!url) return undefined
  const match = url.match(/\/pull\/(\d+)/)
  return match ? parseInt(match[1], 10) : undefined
}

// JSON types from wt list --format json
interface WorktreeJSON {
  branch: string
  path: string
  kind?: string
  base_branch?: string
  working_tree?: {
    staged?: boolean
    modified?: boolean
    untracked?: boolean
  }
  main?: {
    ahead?: number
    behind?: number
  }
  remote?: {
    ahead?: number
    behind?: number
  }
  operation_state?: string
  ci?: {
    source?: string
    url?: string
    state?: string
  }
  prunable?: boolean
}
