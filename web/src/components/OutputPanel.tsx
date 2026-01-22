// Output panel showing session results. Maps to swift/Concerto/Views/OutputPanel.swift

'use client'

export default function OutputPanel() {
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
      }}>
        <h3 style={{ margin: 0, fontSize: '0.875rem', fontWeight: 600 }}>Output</h3>
      </header>

      <div style={{
        flex: 1,
        overflow: 'auto',
        padding: '1rem',
        fontFamily: 'monospace',
        fontSize: '0.8125rem',
        color: 'var(--muted)',
      }}>
        <p>No active sessions</p>
      </div>
    </aside>
  )
}
