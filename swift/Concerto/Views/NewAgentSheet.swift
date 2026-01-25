// Sheet for creating a new agent with optional name and area typeahead.

import SwiftUI
import LoopflowCore

// MARK: - Area Token Chip

struct AreaChip: View {
    let path: String
    let onRemove: () -> Void

    @State private var isHovered = false

    private var displayPath: String {
        if path == "." { return "." }
        // Truncate long paths: show last 30 chars with ellipsis
        if path.count > 30 {
            return "…" + String(path.suffix(28))
        }
        return path
    }

    var body: some View {
        HStack(spacing: 4) {
            Image(systemName: path == "." ? "house" : "folder")
                .font(.system(size: 10))
                .foregroundStyle(.secondary)

            Text(displayPath)
                .font(.system(size: 12))
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
        .background(Color.primary.opacity(0.08))
        .clipShape(RoundedRectangle(cornerRadius: 4))
        .onHover { isHovered = $0 }
        .help(path) // Full path on hover
    }
}

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
        field.isBordered = true
        field.bezelStyle = .roundedBezel
        field.focusRingType = .exterior
        field.placeholderString = placeholder
        field.delegate = context.coordinator
        field.font = .systemFont(ofSize: 13)
        return field
    }

    func updateNSView(_ nsView: NSTextField, context: Context) {
        if nsView.stringValue != text {
            nsView.stringValue = text
        }

        // Update ghost text as attributed string
        let fullText = NSMutableAttributedString(
            string: text,
            attributes: [.foregroundColor: NSColor.labelColor]
        )

        if !ghost.isEmpty && !text.isEmpty {
            let ghostPart = NSAttributedString(
                string: ghost,
                attributes: [.foregroundColor: NSColor.tertiaryLabelColor]
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
            // Extract only the real text (not ghost)
            let fullString = field.stringValue
            if fullString.hasPrefix(parent.text) && fullString.count > parent.text.count {
                // User might have accepted ghost somehow, update text
                parent.text = String(fullString.prefix(while: { _ in true }))
            } else {
                parent.text = fullString
            }
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

// MARK: - Suggestion Chip

struct SuggestionChip: View {
    let path: String
    let isDirectory: Bool
    let onSelect: () -> Void

    @State private var isHovered = false

    private var displayName: String {
        if path == "." { return ". (root)" }
        let name = (path as NSString).lastPathComponent
        return isDirectory ? name + "/" : name
    }

    var body: some View {
        Button(action: onSelect) {
            HStack(spacing: 4) {
                Image(systemName: path == "." ? "house" : "folder")
                    .font(.system(size: 10))
                    .foregroundStyle(.secondary)

                Text(displayName)
                    .font(.system(size: 12))
                    .lineLimit(1)
            }
            .padding(.horizontal, 8)
            .padding(.vertical, 4)
            .background(isHovered ? Color.primary.opacity(0.1) : Color.primary.opacity(0.04))
            .clipShape(RoundedRectangle(cornerRadius: 4))
        }
        .buttonStyle(.plain)
        .onHover { isHovered = $0 }
        .help(path)
    }
}

// MARK: - Token Area Picker

struct TokenAreaPicker: View {
    let repoURL: URL?
    let excludePatterns: [String]
    @Binding var selectedAreas: [String]

    @State private var inputText = ""
    @FocusState private var isFocused: Bool

    // Get children of a directory path
    private func childrenOf(_ parentPath: String) -> [String] {
        guard let repo = repoURL else { return [] }

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

            var isExcluded = false
            for pattern in excludePatterns {
                if name == pattern || pattern.hasPrefix(name + "/") {
                    isExcluded = true
                    break
                }
            }
            guard !isExcluded else { continue }

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
        guard let repo = repoURL else { return false }
        if path == "." { return true }
        let url = repo.appendingPathComponent(path)
        var isDir: ObjCBool = false
        return FileManager.default.fileExists(atPath: url.path, isDirectory: &isDir) && isDir.boolValue
    }

    // Find ghost completion for current input
    private var ghostCompletion: String {
        if inputText.isEmpty { return "" }

        // Parse current input: "src/au" -> parent="src", partial="au"
        let components = inputText.split(separator: "/", omittingEmptySubsequences: false)
        let partial = String(components.last ?? "")
        let parentPath = components.dropLast().joined(separator: "/")

        // Get children of parent
        let candidates: [String]
        if parentPath.isEmpty {
            // Top level: include "." and root folders
            candidates = ["."] + childrenOf(".")
        } else {
            candidates = childrenOf(parentPath)
        }

        // Find first match starting with partial
        for candidate in candidates {
            let candidateName = (candidate as NSString).lastPathComponent
            if candidateName.lowercased().hasPrefix(partial.lowercased()) && candidateName != partial {
                // Return the completion part only
                return String(candidateName.dropFirst(partial.count))
            }
        }

        return ""
    }

    // Suggestions to show below the input
    private var suggestions: [String] {
        // Parse current input
        let components = inputText.split(separator: "/", omittingEmptySubsequences: false)
        let partial = String(components.last ?? "")
        let parentPath = components.dropLast().joined(separator: "/")

        // Get candidates
        var candidates: [String]
        if inputText.isEmpty {
            candidates = ["."] + childrenOf(".")
        } else if inputText == "." {
            candidates = ["."]
        } else {
            candidates = childrenOf(parentPath.isEmpty ? "." : parentPath)
        }

        // Filter by partial match
        if !partial.isEmpty {
            candidates = candidates.filter {
                let name = ($0 as NSString).lastPathComponent
                return name.lowercased().contains(partial.lowercased())
            }
        }

        // Exclude already selected
        candidates = candidates.filter { !selectedAreas.contains($0) }

        return Array(candidates.prefix(8))
    }

    private func acceptGhost() {
        guard !ghostCompletion.isEmpty else {
            // No ghost, but if input is valid path, maybe descend
            if isDirectory(inputText) && !inputText.hasSuffix("/") && inputText != "." {
                inputText += "/"
            }
            return
        }

        inputText += ghostCompletion

        // If it's a directory, add slash to continue
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
        }
        inputText = ""
    }

    private func removeLastToken() {
        guard !selectedAreas.isEmpty else { return }
        selectedAreas.removeLast()
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            // Token input area
            HStack(spacing: 4) {
                // Existing chips
                ForEach(selectedAreas, id: \.self) { area in
                    AreaChip(path: area) {
                        selectedAreas.removeAll { $0 == area }
                    }
                }

                // Ghost text input
                GhostTextField(
                    text: $inputText,
                    ghost: ghostCompletion,
                    placeholder: selectedAreas.isEmpty ? "." : "",
                    onTab: acceptGhost,
                    onEnter: commitToken,
                    onBackspace: removeLastToken
                )
                .frame(minWidth: 60, maxWidth: .infinity)
                .frame(height: 22)
            }
            .padding(6)
            .background(Color(nsColor: .textBackgroundColor))
            .clipShape(RoundedRectangle(cornerRadius: 6))
            .overlay(
                RoundedRectangle(cornerRadius: 6)
                    .stroke(Color.primary.opacity(0.15), lineWidth: 1)
            )

            // Suggestions below
            if !suggestions.isEmpty {
                FlowLayout(spacing: 4) {
                    ForEach(suggestions, id: \.self) { path in
                        SuggestionChip(
                            path: path,
                            isDirectory: isDirectory(path)
                        ) {
                            if !selectedAreas.contains(path) {
                                selectedAreas.append(path)
                            }
                        }
                    }
                }
            }
        }
    }
}

// MARK: - Flow Layout (for wrapping chips)

struct FlowLayout: Layout {
    var spacing: CGFloat = 4

    func sizeThatFits(proposal: ProposedViewSize, subviews: Subviews, cache: inout ()) -> CGSize {
        let result = arrangeSubviews(proposal: proposal, subviews: subviews)
        return result.size
    }

    func placeSubviews(in bounds: CGRect, proposal: ProposedViewSize, subviews: Subviews, cache: inout ()) {
        let result = arrangeSubviews(proposal: proposal, subviews: subviews)

        for (index, subview) in subviews.enumerated() {
            guard index < result.positions.count else { continue }
            let position = result.positions[index]
            subview.place(at: CGPoint(x: bounds.minX + position.x, y: bounds.minY + position.y), proposal: .unspecified)
        }
    }

    private func arrangeSubviews(proposal: ProposedViewSize, subviews: Subviews) -> (size: CGSize, positions: [CGPoint]) {
        let maxWidth = proposal.width ?? .infinity
        var positions: [CGPoint] = []
        var currentX: CGFloat = 0
        var currentY: CGFloat = 0
        var lineHeight: CGFloat = 0
        var totalWidth: CGFloat = 0

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
            totalWidth = max(totalWidth, currentX - spacing)
        }

        return (CGSize(width: totalWidth, height: currentY + lineHeight), positions)
    }
}

// MARK: - New Agent Sheet

struct NewAgentSheet: View {
    @Bindable var appState: AppState
    @Environment(\.dismiss) private var dismiss

    @State private var description = ""
    @State private var name = ""
    @State private var selectedAreas: [String] = []
    @State private var isCreating = false
    @State private var errorMessage: String?
    @FocusState private var isDescriptionFocused: Bool

    var body: some View {
        VStack(spacing: 20) {
            Text("New Agent")
                .font(.headline)

            VStack(alignment: .leading, spacing: 16) {
                // Description field
                VStack(alignment: .leading, spacing: 6) {
                    Text("What do you want to work on?")
                        .font(.caption)
                        .foregroundStyle(.secondary)

                    TextField("Add OAuth login, fix the login bug, etc.", text: $description)
                        .textFieldStyle(.roundedBorder)
                        .focused($isDescriptionFocused)
                }

                // Name field (optional)
                VStack(alignment: .leading, spacing: 6) {
                    HStack {
                        Text("Name")
                            .font(.caption)
                            .foregroundStyle(.secondary)
                        Text("(optional)")
                            .font(.caption2)
                            .foregroundStyle(.tertiary)
                    }

                    TextField("Auto-generates if empty", text: $name)
                        .textFieldStyle(.roundedBorder)
                }

                // Area picker with tokens
                VStack(alignment: .leading, spacing: 6) {
                    Text("Area")
                        .font(.caption)
                        .foregroundStyle(.secondary)

                    TokenAreaPicker(
                        repoURL: appState.currentRepo,
                        excludePatterns: appState.config?.exclude ?? [],
                        selectedAreas: $selectedAreas
                    )
                }
            }

            if let error = errorMessage {
                Text(error)
                    .font(.caption)
                    .foregroundStyle(.red)
            }

            HStack {
                Button("Cancel") {
                    dismiss()
                }
                .keyboardShortcut(.escape)

                Spacer()

                Button("Create Agent") {
                    createAgent()
                }
                .buttonStyle(.borderedProminent)
                .keyboardShortcut(.defaultAction)
                .disabled(isCreating)
            }
        }
        .padding(24)
        .frame(width: 440)
        .onAppear {
            isDescriptionFocused = true
        }
    }

    private func createAgent() {
        isCreating = true
        errorMessage = nil

        let areas = selectedAreas.isEmpty ? ["."] : selectedAreas

        Task {
            do {
                try await appState.createAgent(
                    name: name,
                    description: description,
                    areas: areas
                )
                dismiss()
            } catch {
                errorMessage = error.localizedDescription
            }
            isCreating = false
        }
    }
}

#Preview {
    NewAgentSheet(appState: AppState())
}
