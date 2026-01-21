// Loop status indicator. Maps to LoopflowSwift/Concerto/Views/LoopRow.swift

'use client'

import type { Loop, LoopStatus as LoopStatusType } from '@/models/loop'

interface Props {
  loop: Loop
}

const statusColors: Record<LoopStatusType, string> = {
  idle: '#666',
  running: '#22c55e',
  waiting: '#3b82f6',
  error: '#ef4444',
}

export default function LoopStatus({ loop }: Props) {
  return (
    <div style={{
      display: 'flex',
      alignItems: 'center',
      gap: '0.5rem',
      padding: '0.5rem 0.75rem',
      background: 'rgba(0,0,0,0.03)',
      borderRadius: '6px',
    }}>
      <div style={{
        width: '8px',
        height: '8px',
        borderRadius: '50%',
        background: statusColors[loop.status],
      }} />
      <span style={{ fontSize: '0.875rem', fontWeight: 500 }}>
        {loop.shortId}
      </span>
      <span style={{ fontSize: '0.75rem', color: 'var(--muted)' }}>
        {loop.statusText}
      </span>
    </div>
  )
}
