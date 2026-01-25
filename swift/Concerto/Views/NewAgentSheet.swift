// Sheet for creating a new agent with optional name and area typeahead.

import SwiftUI
import LoopflowCore

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

        var areas = ["."]
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

    private var filteredAreas: [String] {
        if areaSearchText.isEmpty || areaSearchText == "." {
            return availableAreas
        }
        return availableAreas.filter {
            $0.lowercased().contains(areaSearchText.lowercased())
        }
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

                    ZStack(alignment: .topLeading) {
                        TextField(".", text: $areaSearchText)
                            .textFieldStyle(.roundedBorder)
                            .focused($isAreaFocused)
                            .onChange(of: isAreaFocused) { _, focused in
                                isAreaDropdownVisible = focused
                            }
                            .onSubmit {
                                if let first = filteredAreas.first {
                                    areaSearchText = first
                                }
                                isAreaDropdownVisible = false
                            }

                        if isAreaDropdownVisible && !filteredAreas.isEmpty {
                            areaDropdown
                        }
                    }
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

    private var areaDropdown: some View {
        VStack(alignment: .leading, spacing: 0) {
            ForEach(filteredAreas.prefix(8), id: \.self) { area in
                Button {
                    areaSearchText = area
                    isAreaDropdownVisible = false
                    isAreaFocused = false
                } label: {
                    HStack {
                        if area == "." {
                            Text(". (root)")
                        } else {
                            Text(area)
                        }
                        Spacer()
                    }
                    .padding(.horizontal, 10)
                    .padding(.vertical, 6)
                    .contentShape(Rectangle())
                }
                .buttonStyle(.plain)
            }
        }
        .background(.regularMaterial)
        .clipShape(RoundedRectangle(cornerRadius: 6))
        .overlay(
            RoundedRectangle(cornerRadius: 6)
                .stroke(Color.gray.opacity(0.3), lineWidth: 1)
        )
        .shadow(color: .black.opacity(0.1), radius: 4, y: 2)
        .frame(width: 200)
        .offset(y: 34)
        .zIndex(100)
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
