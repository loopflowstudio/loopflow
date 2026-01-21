// Sidebar showing worktrees. Maps to LoopflowSwift/Concerto/Views/WorktreeSidebar.swift

'use client'

import { useState, useEffect } from 'react'
import type { Worktree } from '@/models/worktree'
import { WorktreeService } from '@/services/worktree-service'

interface Props {
  repoPath: string
}

export default function WorktreeSidebar({ repoPath }: Props) {
  const [worktrees, setWorktrees] = useState<Worktree[]>([])
  const [selected, setSelected] = useState<string | null>(null)
  const [loading, setLoading] = useState(true)

  useEffect(() => {
    const service = new WorktreeService(repoPath)
    service.list().then((wts) => {
      setWorktrees(wts)
      setLoading(false)
    }).catch(() => {
      setLoading(false)
    })
  }, [repoPath])

  return (
    <aside style={{
      display: 'flex',
      flexDirection: 'column',
      height: '100%',
      background: 'var(--background)',
    }}>
      <header style={{
        padding: '1rem',
        borderBottom: '1px solid var(--border)',
        display: 'flex',
        justifyContent: 'space-between',
        alignItems: 'center',
      }}>
        <h3 style={{ margin: 0, fontSize: '0.875rem', fontWeight: 600 }}>Worktrees</h3>
        <button style={{
          background: 'none',
          border: 'none',
          cursor: 'pointer',
          fontSize: '1.25rem',
        }}>+</button>
      </header>

      <div style={{ flex: 1, overflow: 'auto' }}>
        {loading ? (
          <p style={{ padding: '1rem', color: 'var(--muted)' }}>Loading...</p>
        ) : worktrees.length === 0 ? (
          <p style={{ padding: '1rem', color: 'var(--muted)' }}>No worktrees</p>
        ) : (
          <ul style={{ listStyle: 'none', padding: 0, margin: 0 }}>
            {worktrees.map((wt) => (
              <li
                key={wt.branch}
                onClick={() => setSelected(wt.branch)}
                style={{
                  padding: '0.75rem 1rem',
                  cursor: 'pointer',
                  background: selected === wt.branch ? 'var(--accent)' : 'transparent',
                  color: selected === wt.branch ? 'white' : 'inherit',
                }}
              >
                <div style={{ fontWeight: 500 }}>{wt.shortName}</div>
                <div style={{
                  fontSize: '0.75rem',
                  color: selected === wt.branch ? 'rgba(255,255,255,0.7)' : 'var(--muted)',
                }}>
                  {wt.commitsText}
                </div>
              </li>
            ))}
          </ul>
        )}
      </div>
    </aside>
  )
}
