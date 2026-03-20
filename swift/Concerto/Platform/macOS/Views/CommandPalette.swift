// Global command palette for searching and executing actions.
// Activated with Cmd+K or /.

import SwiftUI
import LoopflowCore

struct PaletteAction: Identifiable {
    let id: String
    let title: String
    let shortcut: String?
    let icon: String
    let action: () -> Void

    init(_ title: String, icon: String, shortcut: String? = nil, action: @escaping () -> Void) {
        self.id = title.lowercased().replacingOccurrences(of: " ", with: "-")
        self.title = title
        self.icon = icon
        self.shortcut = shortcut
        self.action = action
    }
}

struct CommandPalette: View {
    @Binding var isPresented: Bool
    let actions: [PaletteAction]

    @Environment(\.palette) private var palette
    @State private var query = ""
    @State private var selectedIndex = 0
    @FocusState private var isSearchFocused: Bool

    private var filteredActions: [PaletteAction] {
        if query.isEmpty {
            return actions
        }
        let q = query.lowercased()
        return actions
            .compactMap { action -> (PaletteAction, Int)? in
                let score = fuzzyScore(query: q, target: action.title.lowercased())
                return score > 0 ? (action, score) : nil
            }
            .sorted { $0.1 > $1.1 }
            .map(\.0)
    }

    /// Score a fuzzy match. Returns 0 for no match.
    /// Higher = better: prefix > word-start > scattered subsequence.
    private func fuzzyScore(query: String, target: String) -> Int {
        guard !query.isEmpty else { return 1 }

        var queryIndex = query.startIndex
        var targetIndex = target.startIndex
        var score = 0
        var prevWasBoundary = true // start of string counts as boundary

        while queryIndex < query.endIndex && targetIndex < target.endIndex {
            let qChar = query[queryIndex]
            let tChar = target[targetIndex]

            if qChar == tChar {
                if queryIndex == query.startIndex && targetIndex == target.startIndex {
                    score += 3 // prefix match
                } else if prevWasBoundary {
                    score += 2 // word-start match
                } else {
                    score += 1 // scattered match
                }
                queryIndex = query.index(after: queryIndex)
            }

            prevWasBoundary = tChar == " " || tChar == "-" || tChar == "_"
            targetIndex = target.index(after: targetIndex)
        }

        return queryIndex == query.endIndex ? score : 0
    }

    var body: some View {
        VStack(spacing: 0) {
            // Search field
            HStack(spacing: Spacing.md) {
                Image(systemName: "magnifyingglass")
                    .foregroundStyle(palette.textSecondary)

                TextField("Search actions...", text: $query)
                    .textFieldStyle(.plain)
                    .font(Typography.body(16))
                    .focused($isSearchFocused)
                    .onSubmit {
                        guard selectedIndex < filteredActions.count else { return }
                        execute(filteredActions[selectedIndex])
                    }

                if !query.isEmpty {
                    Button {
                        query = ""
                    } label: {
                        Image(systemName: "xmark.circle.fill")
                            .foregroundStyle(palette.textSecondary)
                    }
                    .buttonStyle(.plain)
                }

                Text("⌘K /")
                    .font(Typography.caption())
                    .foregroundStyle(palette.textSecondary)
                    .padding(.horizontal, Spacing.sm)
                    .padding(.vertical, Spacing.xxs)
                    .background(palette.surfaceMuted)
                    .clipShape(RoundedRectangle(cornerRadius: CornerRadius.sm))
            }
            .padding(.horizontal, Spacing.lg)
            .padding(.vertical, Spacing.md)

            Divider()

            // Results list
            if filteredActions.isEmpty {
                VStack(spacing: Spacing.sm) {
                    Image(systemName: "magnifyingglass")
                        .font(Typography.sectionTitle())
                        .foregroundStyle(palette.textSecondary)
                    Text("No matching actions")
                        .foregroundStyle(palette.textSecondary)
                }
                .frame(maxWidth: .infinity)
                .padding(.vertical, Spacing.xxxl + Spacing.sm)
            } else {
                ScrollViewReader { proxy in
                    ScrollView {
                        LazyVStack(spacing: Spacing.xxs) {
                            ForEach(Array(filteredActions.enumerated()), id: \.element.id) { index, action in
                                ActionRow(
                                    action: action,
                                    isSelected: index == selectedIndex
                                )
                                .id(index)
                                .onTapGesture {
                                    execute(action)
                                }
                            }
                        }
                        .padding(Spacing.sm)
                    }
                    .frame(maxHeight: 300)
                    .onChange(of: selectedIndex) { _, newIndex in
                        withAnimation(.easeOut(duration: 0.1)) {
                            proxy.scrollTo(newIndex, anchor: .center)
                        }
                    }
                }
            }

            // Footer hint
            HStack(spacing: Spacing.lg) {
                Label("navigate", systemImage: "arrow.up.arrow.down")
                Label("select", systemImage: "return")
                Label("close", systemImage: "escape")
            }
            .font(Typography.caption(10))
            .foregroundStyle(palette.textSecondary)
            .padding(.vertical, Spacing.sm)
        }
        .background(.ultraThinMaterial)
        .clipShape(RoundedRectangle(cornerRadius: CornerRadius.lg))
        .shadow(color: .black.opacity(0.3), radius: 20, y: 10)
        .frame(width: 500)
        .onAppear {
            isSearchFocused = true
            selectedIndex = 0
        }
        .onChange(of: query) { _, _ in
            selectedIndex = 0
        }
        .onKeyPress(.upArrow) {
            moveSelection(-1)
            return .handled
        }
        .onKeyPress(.downArrow) {
            moveSelection(1)
            return .handled
        }
        .onKeyPress(.escape) {
            isPresented = false
            return .handled
        }
    }

    private func moveSelection(_ delta: Int) {
        let count = filteredActions.count
        guard count > 0 else { return }
        selectedIndex = (selectedIndex + delta + count) % count
    }

    private func execute(_ action: PaletteAction) {
        isPresented = false
        action.action()
    }
}

private struct ActionRow: View {
    let action: PaletteAction
    let isSelected: Bool

    @Environment(\.palette) private var palette

    var body: some View {
        HStack(spacing: Spacing.md) {
            Image(systemName: action.icon)
                .frame(width: 20)
                .foregroundStyle(isSelected ? .white : palette.textSecondary)

            Text(action.title)
                .foregroundStyle(isSelected ? .white : palette.text)

            Spacer()

            if let shortcut = action.shortcut {
                Text(shortcut)
                    .font(Typography.caption())
                    .foregroundStyle(isSelected ? .white.opacity(0.7) : palette.textSecondary)
                    .padding(.horizontal, Spacing.sm)
                    .padding(.vertical, Spacing.xxs)
                    .background(isSelected ? Color.white.opacity(0.2) : palette.surfaceMuted)
                    .clipShape(RoundedRectangle(cornerRadius: CornerRadius.sm))
            }
        }
        .padding(.horizontal, Spacing.md)
        .padding(.vertical, Spacing.sm)
        .background(
            RoundedRectangle(cornerRadius: CornerRadius.md)
                .fill(isSelected ? palette.accent : Color.clear)
        )
        .contentShape(Rectangle())
    }
}

#Preview {
    CommandPalette(
        isPresented: .constant(true),
        actions: [
            PaletteAction("New Workspace", icon: "plus.square", shortcut: "⌘N") {},
            PaletteAction("Open Terminal", icon: "terminal", shortcut: "T") {},
            PaletteAction("Open IDE", icon: "curlybraces", shortcut: "I") {},
            PaletteAction("View Diff", icon: "doc.text.magnifyingglass", shortcut: "D") {},
            PaletteAction("Create PR", icon: "arrow.triangle.pull", shortcut: "P") {},
            PaletteAction("Run Step", icon: "play.fill", shortcut: "R") {},
            PaletteAction("Focus Prompt", icon: "text.cursor", shortcut: "⌘L") {},
        ]
    )
    .padding(Spacing.xxxl + Spacing.sm)
    .background(Color.gray.opacity(0.3))
}

