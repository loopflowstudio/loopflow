// Worktree model. Maps to swift/LoopflowCore/Models/Worktree.swift

export type PRState = 'open' | 'merged' | 'closed' | 'draft'

export type CIStatus = 'passing' | 'failing' | 'pending' | 'unknown'

export type Staleness =
  | { kind: 'active' }
  | { kind: 'merged' }
  | { kind: 'remoteDeleted' }
  | { kind: 'inactive'; days: number }

export interface Worktree {
  path: string
  branch: string
  baseBranch?: string
  isDirty: boolean
  aheadMain: number
  behindMain: number
  aheadRemote: number
  behindRemote: number
  prURL?: string
  prNumber?: number
  prState?: PRState
  hasCodeWorkspace: boolean
  isRebasing: boolean
  isMerging: boolean
  staleness: Staleness
  ciStatus?: CIStatus
}

// Computed properties (mirroring Swift)
export function shortName(wt: Worktree): string {
  const parts = wt.path.split('/')
  const dirname = parts[parts.length - 1]
  const dotIndex = dirname.indexOf('.')
  if (dotIndex > 0) {
    return dirname.slice(dotIndex + 1)
  }
  return wt.branch
}

export function commitsText(wt: Worktree): string {
  const parts: string[] = []

  if (wt.prNumber) {
    let prText = `PR #${wt.prNumber}`
    if (wt.prState) {
      prText += ` (${wt.prState})`
    }
    parts.push(prText)
  } else {
    parts.push('no PR')
  }

  if (wt.aheadMain > 0) {
    parts.push(`${wt.aheadMain} ahead`)
  }
  if (wt.behindMain > 0) {
    parts.push(`${wt.behindMain} behind`)
  }

  return parts.join(' \u00b7 ')
}
