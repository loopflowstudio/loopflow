// AreaTypeahead - terminal-style path input with fish-style tab completion and chips.

import SwiftUI
import LoopflowCore

struct AreaTypeahead: View {
    let wave: WaveViewModel
    let onSelect: ([String]) -> Void

    @Environment(RepoState.self) private var repoState
    @Environment(\.palette) private var palette

    @State private var inputText = ""
    @State private var selectedAreas: [String] = []

    private let excludePatterns = ["node_modules", "__pycache__", "build", "dist", ".build", "target", "vendor", ".git"]

    var body: some View {
        VStack(alignment: .leading, spacing: Spacing.sm) {
            Text("Area")
                .font(.caption)
                .foregroundStyle(.secondary)

            HStack(spacing: 4) {
                ForEach(selectedAreas, id: \.self) { area in
                    TypeaheadChip(
                        icon: area == "." ? "house" : "folder",
                        label: displayPath(area),
                        helpText: area
                    ) {
                        selectedAreas.removeAll { $0 == area }
                        commitAreas()
                    }
                }

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
            selectedAreas = wave.area
        }
    }

    // MARK: - Display

    private func displayPath(_ path: String) -> String {
        if path == "." { return "." }
        if path.count > 30 {
            return "…" + String(path.suffix(28))
        }
        return path
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
}

#Preview {
    let repoState = RepoState()
    repoState.currentRepo = URL(fileURLWithPath: "/Users/jack/src/loopflow")

    let wave = WaveViewModel(
        api: Wave(
            id: "test",
            name: "test-wave",
            repo: "/Users/jack/src/loopflow",
            flow: "design",
            direction: [],
            area: ["src/loopflow"]
        ),
        recentSteps: []
    )

    return AreaTypeahead(wave: wave) { areas in
        print("Selected: \(areas)")
    }
    .environment(repoState)
    .frame(width: 400)
    .padding()
}
