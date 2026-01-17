// Data structures for context preview panel.

import SwiftUI

enum ContextKind: String, CaseIterable {
    case docs
    case files
    case diff
    case clipboard
    case attached

    var color: Color {
        switch self {
        case .docs: return .blue
        case .files: return .teal
        case .diff: return .green
        case .clipboard: return .purple
        case .attached: return .orange
        }
    }

    var icon: String {
        switch self {
        case .docs: return "doc.text"
        case .files: return "doc.on.doc"
        case .diff: return "plus.forwardslash.minus"
        case .clipboard: return "doc.on.clipboard"
        case .attached: return "paperclip"
        }
    }

    var displayName: String {
        switch self {
        case .docs: return "Docs"
        case .files: return "Files"
        case .diff: return "Diff"
        case .clipboard: return "Clipboard"
        case .attached: return "Attached"
        }
    }
}

struct ContextItem: Identifiable, Equatable {
    let id = UUID()
    let name: String
    let preview: String?
    let tokens: Int
    let path: String?

    static func == (lhs: ContextItem, rhs: ContextItem) -> Bool {
        lhs.id == rhs.id
    }
}

struct ContextSection: Identifiable {
    let id = UUID()
    let kind: ContextKind
    var items: [ContextItem]
    var isEnabled: Bool

    var tokens: Int {
        items.reduce(0) { $0 + $1.tokens }
    }
}

struct ContextPreview {
    var sections: [ContextSection]

    var totalTokens: Int {
        sections.filter { $0.isEnabled }.reduce(0) { $0 + $1.tokens }
    }

    static let empty = ContextPreview(sections: [])
}

// Bundles context-related options to reduce parameter passing
struct ContextOptions {
    let prompt: String?
    let args: String
    let contextFolders: [URL]
    let attachedFiles: [URL]
    let includeDocs: Bool
    let includeDiff: Bool
    let includeDiffFiles: Bool
    let includePaste: Bool
    let includeSummaries: Bool
    let repoURL: URL
}
