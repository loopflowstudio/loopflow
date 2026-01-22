// Data structures for context preview panel.

import SwiftUI

public enum ContextKind: String, Sendable, CaseIterable {
    case docs
    case files
    case diff
    case clipboard
    case attached

    public var color: Color {
        switch self {
        case .docs: return .blue
        case .files: return .teal
        case .diff: return .green
        case .clipboard: return .purple
        case .attached: return .orange
        }
    }

    public var icon: String {
        switch self {
        case .docs: return "doc.text"
        case .files: return "doc.on.doc"
        case .diff: return "plus.forwardslash.minus"
        case .clipboard: return "doc.on.clipboard"
        case .attached: return "paperclip"
        }
    }

    public var displayName: String {
        switch self {
        case .docs: return "Docs"
        case .files: return "Files"
        case .diff: return "Diff"
        case .clipboard: return "Clipboard"
        case .attached: return "Attached"
        }
    }
}

public struct ContextItem: Sendable, Identifiable, Equatable {
    public let id = UUID()
    public let name: String
    public let preview: String?
    public let tokens: Int
    public let path: String?

    public static func == (lhs: ContextItem, rhs: ContextItem) -> Bool {
        lhs.id == rhs.id
    }
}

public struct ContextSection: Sendable, Identifiable {
    public let id = UUID()
    public let kind: ContextKind
    public var items: [ContextItem]
    public var isEnabled: Bool

    public var tokens: Int {
        items.reduce(0) { $0 + $1.tokens }
    }
}

public struct ContextPreview: Sendable {
    public var sections: [ContextSection]

    public var totalTokens: Int {
        sections.filter { $0.isEnabled }.reduce(0) { $0 + $1.tokens }
    }

    public static let empty = ContextPreview(sections: [])
}

// Bundles context-related options to reduce parameter passing
public struct ContextOptions: Sendable {
    public let prompt: String?
    public let args: String
    public let contextFolders: [URL]
    public let attachedFiles: [URL]
    public let includeDocs: Bool
    public let includeDiff: Bool
    public let includeDiffFiles: Bool
    public let includePaste: Bool
    public let includeSummaries: Bool
    public let repoURL: URL

    public init(
        prompt: String?,
        args: String,
        contextFolders: [URL],
        attachedFiles: [URL],
        includeDocs: Bool,
        includeDiff: Bool,
        includeDiffFiles: Bool,
        includePaste: Bool,
        includeSummaries: Bool,
        repoURL: URL
    ) {
        self.prompt = prompt
        self.args = args
        self.contextFolders = contextFolders
        self.attachedFiles = attachedFiles
        self.includeDocs = includeDocs
        self.includeDiff = includeDiff
        self.includeDiffFiles = includeDiffFiles
        self.includePaste = includePaste
        self.includeSummaries = includeSummaries
        self.repoURL = repoURL
    }
}
