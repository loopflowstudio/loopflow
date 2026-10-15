import Loopflow

/// The outer layout places whole Work workspaces. Pane layouts stay inside each workspace.
indirect enum WorkLayout: Equatable {
    case workspace(WorkWorkspaceKey)
    case split(SplitAxis, WorkLayout, WorkLayout, Double)

    var keys: [WorkWorkspaceKey] {
        switch self {
        case .workspace(let key): [key]
        case .split(_, let first, let second, _): first.keys + second.keys
        }
    }

    func replacing(_ key: WorkWorkspaceKey, with replacement: WorkLayout) -> WorkLayout {
        switch self {
        case .workspace(let current): current == key ? replacement : self
        case .split(let axis, let first, let second, let ratio):
            .split(axis, first.replacing(key, with: replacement), second.replacing(key, with: replacement), ratio)
        }
    }

    func removing(_ key: WorkWorkspaceKey) -> WorkLayout? {
        switch self {
        case .workspace(let current): return current == key ? nil : self
        case .split(let axis, let first, let second, let ratio):
            switch (first.removing(key), second.removing(key)) {
            case let (first?, second?): return .split(axis, first, second, ratio)
            case let (first?, nil): return first
            case let (nil, second?): return second
            case (nil, nil): return nil
            }
        }
    }

    func resizing(between firstKey: WorkWorkspaceKey, and secondKey: WorkWorkspaceKey, ratio: Double) -> WorkLayout {
        switch self {
        case .workspace: return self
        case .split(let axis, let first, let second, let current):
            if first.keys.contains(firstKey), second.keys.contains(secondKey) {
                return .split(axis, first, second, min(0.9, max(0.1, ratio)))
            }
            return .split(axis, first.resizing(between: firstKey, and: secondKey, ratio: ratio),
                          second.resizing(between: firstKey, and: secondKey, ratio: ratio), current)
        }
    }
}
