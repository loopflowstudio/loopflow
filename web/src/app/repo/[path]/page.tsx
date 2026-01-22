'use client'

import { use } from 'react'
import WorktreeSidebar from '@/components/WorktreeSidebar'
import PromptLauncher from '@/components/PromptLauncher'
import OutputPanel from '@/components/OutputPanel'

interface Props {
  params: Promise<{ path: string }>
}

export default function RepoPage({ params }: Props) {
  const { path } = use(params)
  const repoPath = decodeURIComponent(path)

  return (
    <div style={{
      display: 'grid',
      gridTemplateColumns: '280px 1fr 400px',
      height: '100vh',
    }}>
      <WorktreeSidebar repoPath={repoPath} />

      <main style={{
        display: 'flex',
        flexDirection: 'column',
        borderLeft: '1px solid var(--border)',
        borderRight: '1px solid var(--border)',
      }}>
        <header style={{
          padding: '1rem',
          borderBottom: '1px solid var(--border)',
        }}>
          <h2 style={{ fontSize: '1.25rem', margin: 0 }}>{repoPath.split('/').pop()}</h2>
          <p style={{ color: 'var(--muted)', fontSize: '0.875rem' }}>{repoPath}</p>
        </header>

        <div style={{ flex: 1, overflow: 'auto', padding: '1rem' }}>
          <PromptLauncher repoPath={repoPath} />
        </div>
      </main>

      <OutputPanel />
    </div>
  )
}
