// Flow model. Maps to swift/LoopflowCore/Models/Flow.swift

import type { Step } from './step'

// FlowStep = a Step with its parent Flow
export interface FlowStep extends Step {
  flow: string    // the flow this step belongs to
}

export interface FlowDef {
  id: string
  name: string
  steps: Step[]
}
