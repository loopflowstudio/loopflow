// Global command palette for searching and executing actions.
// Activated with Cmd+K.

import SwiftUI

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

    @State private var query = ""
    @State private var selectedIndex = 0
    @FocusState private var isSearchFocused: Bool

    private var filteredActions: [PaletteAction] {
        if query.isEmpty {
            return actions
        }
        return actions.filter { action in
            fuzzyMatch(query: query.lowercased(), target: action.title.lowercased())
        }
    }

    private func fuzzyMatch(query: String, target: String) -> Bool {
        var queryIndex = query.startIndex
        var targetIndex = target.startIndex

        while queryIndex < query.endIndex && targetIndex < target.endIndex {
            if query[queryIndex] == target[targetIndex] {
                queryIndex = query.index(after: queryIndex)
            }
            targetIndex = target.index(after: targetIndex)
        }

        return queryIndex == query.endIndex
    }

    var body: some View {
        VStack(spacing: 0) {
            // Search field
            HStack(spacing: 12) {
                Image(systemName: "magnifyingglass")
                    .foregroundStyle(.secondary)

                TextField("Search actions...", text: $query)
                    .textFieldStyle(.plain)
                    .font(.system(size: 16))
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
                            .foregroundStyle(.secondary)
                    }
                    .buttonStyle(.plain)
                }

                Text("⌘K")
                    .font(.caption)
                    .foregroundStyle(.tertiary)
                    .padding(.horizontal, 6)
                    .padding(.vertical, 2)
                    .background(Color.secondary.opacity(0.2))
                    .clipShape(RoundedRectangle(cornerRadius: 4))
            }
            .padding(.horizontal, 16)
            .padding(.vertical, 12)

            Divider()

            // Results list
            if filteredActions.isEmpty {
                VStack(spacing: 8) {
                    Image(systemName: "magnifyingglass")
                        .font(.title2)
                        .foregroundStyle(.tertiary)
                    Text("No matching actions")
                        .foregroundStyle(.secondary)
                }
                .frame(maxWidth: .infinity)
                .padding(.vertical, 40)
            } else {
                ScrollViewReader { proxy in
                    ScrollView {
                        LazyVStack(spacing: 2) {
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
                        .padding(8)
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
            HStack(spacing: 16) {
                Label("navigate", systemImage: "arrow.up.arrow.down")
                Label("select", systemImage: "return")
                Label("close", systemImage: "escape")
            }
            .font(.caption2)
            .foregroundStyle(.tertiary)
            .padding(.vertical, 8)
        }
        .background(.ultraThinMaterial)
        .clipShape(RoundedRectangle(cornerRadius: 12))
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

    var body: some View {
        HStack(spacing: 12) {
            Image(systemName: action.icon)
                .frame(width: 20)
                .foregroundStyle(isSelected ? .white : .secondary)

            Text(action.title)
                .foregroundStyle(isSelected ? .white : .primary)

            Spacer()

            if let shortcut = action.shortcut {
                Text(shortcut)
                    .font(.caption)
                    .foregroundStyle(isSelected ? .white.opacity(0.7) : .secondary)
                    .padding(.horizontal, 6)
                    .padding(.vertical, 2)
                    .background(isSelected ? Color.white.opacity(0.2) : Color.secondary.opacity(0.15))
                    .clipShape(RoundedRectangle(cornerRadius: 4))
            }
        }
        .padding(.horizontal, 12)
        .padding(.vertical, 8)
        .background(
            RoundedRectangle(cornerRadius: 8)
                .fill(isSelected ? Color.accentColor : Color.clear)
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
    .padding(40)
    .background(Color.gray.opacity(0.3))
}
