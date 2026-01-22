'use client'

import { useState } from 'react'
import { useRouter } from 'next/navigation'

export default function WelcomePage() {
  const [repoPath, setRepoPath] = useState('')
  const router = useRouter()

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault()
    if (repoPath) {
      // URL-encode the path for the route
      router.push(`/repo/${encodeURIComponent(repoPath)}`)
    }
  }

  return (
    <main style={{
      display: 'flex',
      flexDirection: 'column',
      alignItems: 'center',
      justifyContent: 'center',
      minHeight: '100vh',
      padding: '2rem',
    }}>
      <h1 style={{ marginBottom: '0.5rem' }}>Loopflow</h1>
      <p style={{ color: 'var(--muted)', marginBottom: '2rem' }}>
        Development workflow orchestration
      </p>

      <form onSubmit={handleSubmit} style={{ width: '100%', maxWidth: '400px' }}>
        <input
          type="text"
          placeholder="Enter repository path..."
          value={repoPath}
          onChange={(e) => setRepoPath(e.target.value)}
          style={{
            width: '100%',
            padding: '0.75rem 1rem',
            fontSize: '1rem',
            border: '1px solid var(--border)',
            borderRadius: '8px',
            background: 'var(--background)',
            color: 'var(--foreground)',
            marginBottom: '1rem',
          }}
        />
        <button
          type="submit"
          style={{
            width: '100%',
            padding: '0.75rem 1rem',
            fontSize: '1rem',
            background: 'var(--accent)',
            color: 'white',
            border: 'none',
            borderRadius: '8px',
            cursor: 'pointer',
          }}
        >
          Open Repository
        </button>
      </form>

      <p style={{
        marginTop: '3rem',
        color: 'var(--muted)',
        fontSize: '0.875rem',
        textAlign: 'center',
        maxWidth: '400px',
      }}>
        Web client follows the Swift app design. Connect to lfd for local mode or Symphonia API for teams mode.
      </p>
    </main>
  )
}
