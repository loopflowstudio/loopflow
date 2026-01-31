// AreaTypeahead - terminal-style path input with fish-style tab completion and chips.

import SwiftUI
import LoopflowCore
import AppKit

// MARK: - Ghost Text Field (Fish-style autocomplete)

struct GhostTextField: NSViewRepresentable {
    @Binding var text: String
    let ghost: String
    let placeholder: String
    let onTab: () -> Void
    let onEnter: () -> Void
    let onBackspace: () -> Void

    func makeNSView(context: Context) -> NSTextField {
        let field = NSTextField()
        field.isBordered = false
        field.drawsBackground = false
        field.focusRingType = .none
        field.placeholderString = placeholder
        field.delegate = context.coordinator
        field.font = .monospacedSystemFont(ofSize: 13, weight: .regular)
        return field
    }

    func updateNSView(_ nsView: NSTextField, context: Context) {
        if nsView.stringValue != text {
            nsView.stringValue = text
        }

        // Update ghost text as attributed string
        let fullText = NSMutableAttributedString(
            string: text,
            attributes: [
                .foregroundColor: NSColor.labelColor,
                .font: NSFont.monospacedSystemFont(ofSize: 13, weight: .regular)
            ]
        )

        if !ghost.isEmpty && !text.isEmpty {
            let ghostPart = NSAttributedString(
                string: ghost,
                attributes: [
                    .foregroundColor: NSColor.tertiaryLabelColor,
                    .font: NSFont.monospacedSystemFont(ofSize: 13, weight: .regular)
                ]
            )
            fullText.append(ghostPart)
        }

        // Only update if different to avoid cursor jumping
        if nsView.attributedStringValue.string != fullText.string {
            nsView.attributedStringValue = fullText
            // Keep cursor at end of real text
            nsView.currentEditor()?.selectedRange = NSRange(location: text.count, length: 0)
        }
    }

    func makeCoordinator() -> Coordinator {
        Coordinator(self)
    }

    class Coordinator: NSObject, NSTextFieldDelegate {
        var parent: GhostTextField

        init(_ parent: GhostTextField) {
            self.parent = parent
        }

        func controlTextDidChange(_ obj: Notification) {
            guard let field = obj.object as? NSTextField else { return }
            parent.text = field.stringValue
        }

        func control(_ control: NSControl, textView: NSTextView, doCommandBy commandSelector: Selector) -> Bool {
            if commandSelector == #selector(NSResponder.insertTab(_:)) {
                parent.onTab()
                return true
            }
            if commandSelector == #selector(NSResponder.insertNewline(_:)) {
                parent.onEnter()
                return true
            }
            if commandSelector == #selector(NSResponder.deleteBackward(_:)) {
                if parent.text.isEmpty {
                    parent.onBackspace()
                    return true
                }
            }
            return false
        }
    }
}

// MARK: - Area Chip

struct AreaChip: View {
    let path: String
    let onRemove: () -> Void

    @Environment(\.colorScheme) private var colorScheme
    @State private var isHovered = false

    private var palette: LoopflowPalette { LoopflowPalette.make(for: colorScheme) }

    private var displayPath: String {
        if path == "." { return "." }
        if path.count > 30 {
            return "…" + String(path.suffix(28))
        }
        return path
    }

    var body: some View {
        HStack(spacing: 4) {
            Image(systemName: path == "." ? "house" : "folder")
                .font(.system(size: 10))
                .foregroundStyle(palette.accent)

            Text(displayPath)
                .font(.system(size: 12, design: .monospaced))
                .lineLimit(1)

            Button {
                onRemove()
            } label: {
                Image(systemName: "xmark")
                    .font(.system(size: 9, weight: .semibold))
                    .foregroundStyle(.secondary)
            }
            .buttonStyle(.plain)
            .opacity(isHovered ? 1 : 0.6)
        }
        .padding(.horizontal, 8)
        .padding(.vertical, 4)
        .background(palette.accent.opacity(0.15))
        .clipShape(RoundedRectangle(cornerRadius: 4))
        .onHover { isHovered = $0 }
        .help(path)
    }
}

// MARK: - Area Typeahead

struct AreaTypeahead: View {
    let wave: Wave
    let onSelect: ([String]) -> Void

    @Environment(RepoState.self) private var repoState
    @Environment(\.colorScheme) private var colorScheme

    @State private var inputText = ""
    @State private var selectedAreas: [String] = []
    @State private var directoryCache: [String: [String]] = [:]

    private var palette: LoopflowPalette { LoopflowPalette.make(for: colorScheme) }

    private let excludePatterns = ["node_modules", "__pycache__", "build", "dist", ".build", "target", "vendor", ".git"]

    var body: some View {
        VStack(alignment: .leading, spacing: Spacing.sm) {
            Text("Area")
                .font(.caption)
                .foregroundStyle(.secondary)

            // Token input area
            HStack(spacing: 4) {
                // Existing chips
                ForEach(selectedAreas, id: \.self) { area in
                    AreaChip(path: area) {
                        selectedAreas.removeAll { $0 == area }
                        commitAreas()
                    }
                }

                // Ghost text input
                GhostTextField(
                    text: $inputText,
                    ghost: computeGhostCompletion(for: inputText),
                    placeholder: selectedAreas.isEmpty ? "Type path, tab to complete" : "",
                    onTab: acceptGhost,
                    onEnter: commitToken,
                    onBackspace: removeLastToken
                )
                .frame(minWidth: 80, maxWidth: .infinity)
                .frame(height: 20)
            }
            .padding(Spacing.sm)
            .background(palette.surface)
            .clipShape(RoundedRectangle(cornerRadius: CornerRadius.md))
        }
        .onAppear {
            selectedAreas = wave.area ?? []
            loadDirectoryCache()
        }
    }

    // MARK: - Path Completion Logic

    private func childrenOf(_ parentPath: String) -> [String] {
        guard let repo = repoState.currentRepo else { return [] }

        let parentURL: URL
        if parentPath.isEmpty || parentPath == "." {
            parentURL = repo
        } else {
            parentURL = repo.appendingPathComponent(parentPath)
        }

        let fm = FileManager.default
        guard let contents = try? fm.contentsOfDirectory(
            at: parentURL,
            includingPropertiesForKeys: [.isDirectoryKey]
        ) else {
            return []
        }

        var children: [String] = []
        for item in contents {
            let name = item.lastPathComponent
            guard !name.hasPrefix(".") else { continue }
            guard !excludePatterns.contains(name) else { continue }

            let isDir = (try? item.resourceValues(forKeys: [.isDirectoryKey]))?.isDirectory ?? false
            if isDir {
                let fullPath = parentPath.isEmpty || parentPath == "."
                    ? name
                    : parentPath + "/" + name
                children.append(fullPath)
            }
        }
        return children.sorted()
    }

    private func isDirectory(_ path: String) -> Bool {
        guard let repo = repoState.currentRepo else { return false }
        if path == "." { return true }
        let url = repo.appendingPathComponent(path)
        var isDir: ObjCBool = false
        return FileManager.default.fileExists(atPath: url.path, isDirectory: &isDir) && isDir.boolValue
    }

    /// Compute ghost completion for given input text.
    /// Returns the suffix to append (e.g., "ripts" for input "sc" matching "scripts").
    private func computeGhostCompletion(for text: String) -> String {
        if text.isEmpty { return "" }

        let components = text.split(separator: "/", omittingEmptySubsequences: false)
        let partial = String(components.last ?? "")
        let parentPath = components.dropLast().joined(separator: "/")

        let candidates: [String]
        if parentPath.isEmpty {
            candidates = ["."] + childrenOf(".")
        } else {
            candidates = childrenOf(parentPath)
        }

        for candidate in candidates {
            let candidateName = (candidate as NSString).lastPathComponent
            if candidateName.lowercased().hasPrefix(partial.lowercased()) && candidateName != partial {
                return String(candidateName.dropFirst(partial.count))
            }
        }

        return ""
    }

    // MARK: - Actions

    private func acceptGhost() {
        // Compute ghost inline to avoid stale values
        let ghost = computeGhostCompletion(for: inputText)

        guard !ghost.isEmpty else {
            if isDirectory(inputText) && !inputText.hasSuffix("/") && inputText != "." {
                inputText += "/"
            }
            return
        }

        inputText += ghost

        if isDirectory(inputText) && inputText != "." {
            inputText += "/"
        }
    }

    private func commitToken() {
        let path = inputText.trimmingCharacters(in: CharacterSet(charactersIn: "/"))
        guard !path.isEmpty else { return }

        let finalPath = path.isEmpty ? "." : path

        if !selectedAreas.contains(finalPath) {
            selectedAreas.append(finalPath)
            commitAreas()
        }
        inputText = ""
    }

    private func removeLastToken() {
        guard !selectedAreas.isEmpty else { return }
        selectedAreas.removeLast()
        commitAreas()
    }

    private func commitAreas() {
        onSelect(selectedAreas)
    }

    private func loadDirectoryCache() {
        // Pre-load top-level directories
        Task {
            _ = childrenOf(".")
        }
    }
}

// MARK: - Flow Layout

struct FlowLayout: Layout {
    var spacing: CGFloat = 4

    func sizeThatFits(proposal: ProposedViewSize, subviews: Subviews, cache: inout ()) -> CGSize {
        let result = arrangeSubviews(proposal: proposal, subviews: subviews)
        return result.size
    }

    func placeSubviews(in bounds: CGRect, proposal: ProposedViewSize, subviews: Subviews, cache: inout ()) {
        let result = arrangeSubviews(proposal: proposal, subviews: subviews)

        for (index, position) in result.positions.enumerated() {
            subviews[index].place(
                at: CGPoint(x: bounds.minX + position.x, y: bounds.minY + position.y),
                proposal: .unspecified
            )
        }
    }

    private func arrangeSubviews(proposal: ProposedViewSize, subviews: Subviews) -> (size: CGSize, positions: [CGPoint]) {
        let maxWidth = proposal.width ?? .infinity
        var positions: [CGPoint] = []
        var currentX: CGFloat = 0
        var currentY: CGFloat = 0
        var lineHeight: CGFloat = 0
        var totalHeight: CGFloat = 0

        for subview in subviews {
            let size = subview.sizeThatFits(.unspecified)

            if currentX + size.width > maxWidth && currentX > 0 {
                currentX = 0
                currentY += lineHeight + spacing
                lineHeight = 0
            }

            positions.append(CGPoint(x: currentX, y: currentY))
            lineHeight = max(lineHeight, size.height)
            currentX += size.width + spacing
            totalHeight = currentY + lineHeight
        }

        return (CGSize(width: maxWidth, height: max(totalHeight, 20)), positions)
    }
}

#Preview {
    let repoState = RepoState()
    repoState.currentRepo = URL(fileURLWithPath: "/Users/jack/src/loopflow")

    let wave = Wave(
        id: "test",
        name: "test-wave",
        area: ["src/loopflow"],
        flow: "design",
        repo: "/Users/jack/src/loopflow",
        recentSteps: []
    )

    return AreaTypeahead(wave: wave) { areas in
        print("Selected: \(areas)")
    }
    .environment(repoState)
    .frame(width: 400)
    .padding()
}
