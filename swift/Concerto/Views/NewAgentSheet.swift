// Sheet for creating a new agent with optional name and area typeahead.

import SwiftUI
import LoopflowCore

// MARK: - Typeahead Item

struct TypeaheadItem: Identifiable, Equatable {
    let id: String
    let label: String
    let icon: String
    let section: String
    var subtitle: String?
}

// MARK: - Typeahead Dropdown

struct TypeaheadDropdown: View {
    let items: [TypeaheadItem]
    let selectedId: String?
    let onSelect: (TypeaheadItem) -> Void

    @State private var hoveredId: String?

    private var groupedItems: [(section: String, items: [TypeaheadItem])] {
        var sections: [String] = []
        var groups: [String: [TypeaheadItem]] = [:]

        for item in items {
            if groups[item.section] == nil {
                sections.append(item.section)
                groups[item.section] = []
            }
            groups[item.section]?.append(item)
        }

        return sections.compactMap { section in
            guard let items = groups[section] else { return nil }
            return (section: section, items: items)
        }
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            ForEach(Array(groupedItems.enumerated()), id: \.element.section) { index, group in
                if index > 0 {
                    Divider()
                        .padding(.vertical, 4)
                }

                // Section header
                Text(group.section)
                    .font(.caption)
                    .fontWeight(.medium)
                    .foregroundStyle(.secondary)
                    .padding(.horizontal, 12)
                    .padding(.top, index == 0 ? 8 : 4)
                    .padding(.bottom, 4)

                // Items
                ForEach(group.items) { item in
                    TypeaheadRow(
                        item: item,
                        isHovered: hoveredId == item.id,
                        isSelected: selectedId == item.id
                    )
                    .onTapGesture {
                        onSelect(item)
                    }
                    .onHover { hovering in
                        hoveredId = hovering ? item.id : nil
                    }
                }
            }
        }
        .padding(.bottom, 8)
        .background(.regularMaterial)
        .clipShape(RoundedRectangle(cornerRadius: 8))
        .overlay(
            RoundedRectangle(cornerRadius: 8)
                .stroke(Color.primary.opacity(0.1), lineWidth: 1)
        )
        .shadow(color: .black.opacity(0.12), radius: 8, y: 4)
    }
}

struct TypeaheadRow: View {
    let item: TypeaheadItem
    let isHovered: Bool
    let isSelected: Bool

    var body: some View {
        HStack(spacing: 10) {
            Image(systemName: item.icon)
                .font(.system(size: 14))
                .foregroundStyle(.secondary)
                .frame(width: 20)

            Text(item.label)
                .font(.body)

            if let subtitle = item.subtitle {
                Text("·")
                    .foregroundStyle(.tertiary)
                Text(subtitle)
                    .font(.body)
                    .foregroundStyle(.tertiary)
            }

            Spacer()

            if isSelected {
                Image(systemName: "checkmark")
                    .font(.caption)
                    .foregroundStyle(.secondary)
            }
        }
        .padding(.horizontal, 12)
        .padding(.vertical, 8)
        .background(isHovered ? Color.primary.opacity(0.06) : Color.clear)
        .contentShape(Rectangle())
    }
}

// MARK: - New Agent Sheet

struct NewAgentSheet: View {
    @Bindable var appState: AppState
    @Environment(\.dismiss) private var dismiss

    @State private var description = ""
    @State private var name = ""
    @State private var areaSearchText = "."
    @State private var isAreaDropdownVisible = false
    @State private var isCreating = false
    @State private var errorMessage: String?
    @FocusState private var isDescriptionFocused: Bool
    @FocusState private var isAreaFocused: Bool

    private var availableAreas: [String] {
        guard let repo = appState.currentRepo else { return ["."] }
        let fm = FileManager.default
        guard let contents = try? fm.contentsOfDirectory(
            at: repo,
            includingPropertiesForKeys: [.isDirectoryKey]
        ) else {
            return ["."]
        }

        var areas: [String] = []
        let excludePatterns = appState.config?.exclude ?? []

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
                areas.append(name)
            }
        }

        return areas.sorted()
    }

    private var typeaheadItems: [TypeaheadItem] {
        var items: [TypeaheadItem] = []

        // Root is always first in its own section
        let showRoot = areaSearchText.isEmpty ||
                       areaSearchText == "." ||
                       "root".contains(areaSearchText.lowercased())

        if showRoot {
            items.append(TypeaheadItem(
                id: ".",
                label: ".",
                icon: "house",
                section: "Root",
                subtitle: "entire repository"
            ))
        }

        // Filter folders
        let folders = availableAreas.filter { area in
            if areaSearchText.isEmpty || areaSearchText == "." {
                return true
            }
            return area.lowercased().contains(areaSearchText.lowercased())
        }

        for folder in folders.prefix(7) {
            items.append(TypeaheadItem(
                id: folder,
                label: folder,
                icon: "folder",
                section: "Folders"
            ))
        }

        return items
    }

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

                // Area picker with typeahead
                VStack(alignment: .leading, spacing: 6) {
                    Text("Area")
                        .font(.caption)
                        .foregroundStyle(.secondary)

                    TextField(".", text: $areaSearchText)
                        .textFieldStyle(.roundedBorder)
                        .focused($isAreaFocused)
                        .onChange(of: isAreaFocused) { _, focused in
                            withAnimation(.easeOut(duration: 0.15)) {
                                isAreaDropdownVisible = focused
                            }
                        }
                        .onChange(of: areaSearchText) { _, _ in
                            if !isAreaDropdownVisible && isAreaFocused {
                                isAreaDropdownVisible = true
                            }
                        }
                        .onSubmit {
                            if let first = typeaheadItems.first {
                                areaSearchText = first.id
                            }
                            isAreaDropdownVisible = false
                        }
                        .overlay(alignment: .topLeading) {
                            if isAreaDropdownVisible && !typeaheadItems.isEmpty {
                                TypeaheadDropdown(
                                    items: typeaheadItems,
                                    selectedId: areaSearchText,
                                    onSelect: { item in
                                        areaSearchText = item.id
                                        isAreaDropdownVisible = false
                                        isAreaFocused = false
                                    }
                                )
                                .frame(minWidth: 280)
                                .offset(y: 36)
                            }
                        }
                        .zIndex(100)
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
        .frame(width: 400)
        .onAppear {
            isDescriptionFocused = true
        }
    }

    private func createAgent() {
        isCreating = true
        errorMessage = nil

        Task {
            do {
                try await appState.createAgent(
                    name: name,
                    description: description,
                    area: areaSearchText
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
