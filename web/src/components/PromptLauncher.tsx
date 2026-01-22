// Prompt composer and launcher. Maps to swift/Concerto/Views/PromptLauncher.swift

'use client'

import { useState } from 'react'

interface Props {
  repoPath: string
}

export default function PromptLauncher({ repoPath }: Props) {
  const [input, setInput] = useState('')
  const [isLaunching, setIsLaunching] = useState(false)

  const handleLaunch = async () => {
    if (!input.trim()) return

    setIsLaunching(true)
    // TODO: Connect to lfd or Symphonia API
    console.log('Launch:', input, 'in', repoPath)

    // Simulate delay
    await new Promise(resolve => setTimeout(resolve, 500))
    setIsLaunching(false)
    setInput('')
  }

  return (
    <div style={{ display: 'flex', flexDirection: 'column', gap: '1rem' }}>
      <textarea
        value={input}
        onChange={(e) => setInput(e.target.value)}
        placeholder="What would you like to work on?"
        style={{
          width: '100%',
          minHeight: '120px',
          padding: '1rem',
          fontSize: '1rem',
          border: '1px solid var(--border)',
          borderRadius: '8px',
          resize: 'vertical',
          fontFamily: 'inherit',
          background: 'var(--background)',
          color: 'var(--foreground)',
        }}
        onKeyDown={(e) => {
          if (e.key === 'Enter' && e.metaKey) {
            handleLaunch()
          }
        }}
      />

      <div style={{ display: 'flex', gap: '0.5rem', justifyContent: 'flex-end' }}>
        <button
          onClick={handleLaunch}
          disabled={isLaunching || !input.trim()}
          style={{
            padding: '0.5rem 1.5rem',
            fontSize: '0.875rem',
            background: 'var(--accent)',
            color: 'white',
            border: 'none',
            borderRadius: '6px',
            cursor: isLaunching || !input.trim() ? 'not-allowed' : 'pointer',
            opacity: isLaunching || !input.trim() ? 0.6 : 1,
          }}
        >
          {isLaunching ? 'Launching...' : 'Launch'}
        </button>
      </div>

      <p style={{
        fontSize: '0.75rem',
        color: 'var(--muted)',
        textAlign: 'right',
      }}>
        Cmd+Enter to launch
      </p>
    </div>
  )
}
