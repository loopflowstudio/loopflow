import Foundation

public struct LoopflowState: Codable, Equatable {
    public var selectedRepoPath: String?

    public init(selectedRepoPath: String? = nil) {
        self.selectedRepoPath = selectedRepoPath
    }
}

public func loadLoopflowState(stateURL: URL? = nil) -> LoopflowState? {
    let resolvedURL = stateURL ?? _defaultStateURL
    guard
        let raw = try? String(contentsOf: resolvedURL, encoding: .utf8),
        !raw.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
    else {
        return nil
    }

    let selectedRepoPath = raw.components(separatedBy: .newlines)
        .compactMap(_selectedRepoPath)
        .first
    return LoopflowState(selectedRepoPath: selectedRepoPath)
}

public func saveLoopflowState(_ state: LoopflowState, stateURL: URL? = nil) throws {
    let resolvedURL = stateURL ?? _defaultStateURL
    try FileManager.default.createDirectory(
        at: resolvedURL.deletingLastPathComponent(),
        withIntermediateDirectories: true
    )

    let selected = state.selectedRepoPath
        .map { "\"\(_escapeYAMLString($0))\"" }
        ?? ""
    try "selected_repo_path: \(selected)\n".write(
        to: resolvedURL,
        atomically: true,
        encoding: .utf8
    )
}

private let _defaultStateURL = URL(
    fileURLWithPath: NSHomeDirectory(),
    isDirectory: true
)
.appendingPathComponent(".lf", isDirectory: true)
.appendingPathComponent("loopflow-state.yaml", isDirectory: false)

private func _selectedRepoPath(_ line: String) -> String? {
    guard line.first != " ", line.first != "\t" else { return nil }
    let trimmed = line.trimmingCharacters(in: .whitespaces)
    guard !trimmed.hasPrefix("#"), let separator = trimmed.firstIndex(of: ":") else {
        return nil
    }
    guard trimmed[..<separator] == "selected_repo_path" else { return nil }
    let value = trimmed[trimmed.index(after: separator)...]
        .trimmingCharacters(in: .whitespaces)
    guard !value.isEmpty else { return nil }
    if value.count >= 2, value.hasPrefix("\""), value.hasSuffix("\"") {
        return String(value.dropFirst().dropLast())
            .replacingOccurrences(of: "\\\"", with: "\"")
            .replacingOccurrences(of: "\\\\", with: "\\")
    }
    if value.count >= 2, value.hasPrefix("'"), value.hasSuffix("'") {
        return String(value.dropFirst().dropLast())
    }
    return value
}

private func _escapeYAMLString(_ value: String) -> String {
    value
        .replacingOccurrences(of: "\\", with: "\\\\")
        .replacingOccurrences(of: "\"", with: "\\\"")
}
