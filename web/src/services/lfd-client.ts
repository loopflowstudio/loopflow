// LFD client for connecting to local lfd daemon.
// Simple HTTP + WebSocket, no fancy optimizations.
// Maps to swift/LoopflowCore's LFDEventService but over HTTP instead of Unix sockets.

export type ConnectionMode = 'local' | 'teams'

interface LfdResponse<T> {
  ok: boolean
  result: T | null
  error: string | null
}

export interface LfdClientConfig {
  mode: ConnectionMode
  baseUrl?: string  // For teams mode
}

const DEFAULT_LOCAL_URL = 'http://localhost:8765'

export class LfdClient {
  private repoPath: string
  private baseUrl: string

  constructor(repoPath: string, config?: LfdClientConfig) {
    this.repoPath = repoPath
    this.baseUrl = config?.baseUrl ?? DEFAULT_LOCAL_URL
  }

  async get<T>(path: string): Promise<T> {
    const url = `${this.baseUrl}${path}?repo=${encodeURIComponent(this.repoPath)}`
    const response = await fetch(url)

    if (!response.ok) {
      throw new Error(`LFD request failed: ${response.status}`)
    }

    const json = await response.json() as LfdResponse<T>
    if (!json.ok) {
      throw new Error(json.error ?? 'LFD request failed')
    }
    return json.result as T
  }

  async post<T>(path: string, body?: unknown): Promise<T> {
    const url = `${this.baseUrl}${path}`
    const response = await fetch(url, {
      method: 'POST',
      headers: {
        'Content-Type': 'application/json',
      },
      body: JSON.stringify({ repo: this.repoPath, ...body }),
    })

    if (!response.ok) {
      throw new Error(`LFD request failed: ${response.status}`)
    }

    const json = await response.json() as LfdResponse<T>
    if (!json.ok) {
      throw new Error(json.error ?? 'LFD request failed')
    }
    return json.result as T
  }

  // WebSocket for real-time events
  connect(onEvent: (event: LfdEvent) => void): () => void {
    const ws = new WebSocket(`${this.baseUrl.replace('http', 'ws')}/ws`)

    ws.onopen = () => {
      // Subscribe to events for this repo
      ws.send(JSON.stringify({
        type: 'subscribe',
        repo: this.repoPath,
        patterns: ['worktree.*', 'session.*', 'loop.*'],
      }))
    }

    ws.onmessage = (event) => {
      try {
        const data = JSON.parse(event.data)
        onEvent(data)
      } catch {
        console.error('Failed to parse LFD event')
      }
    }

    // Return cleanup function
    return () => {
      ws.close()
    }
  }
}

// Event types from lfd
export interface LfdEvent {
  type: 'worktree' | 'session' | 'output' | 'loop'
  name: string
  data: unknown
}
