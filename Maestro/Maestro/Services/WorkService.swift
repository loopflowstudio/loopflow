// Service for managing work items via lfwork CLI.

import Foundation

actor WorkService {
    private let repoPath: String

    init(repoPath: String) {
        self.repoPath = repoPath
    }

    func listItems(status: WorkStatus? = nil) async throws -> [WorkItem] {
        var args = ["list", "--verbose"]
        if let status = status {
            args += ["--status", status.rawValue]
        }

        let output = try await runLfwork(args)
        return parseListOutput(output)
    }

    func approve(_ itemId: String) async throws {
        _ = try await runLfwork(["approve", itemId])
    }

    func reject(_ itemId: String) async throws {
        _ = try await runLfwork(["reject", itemId])
    }

    func claim(_ itemId: String) async throws {
        _ = try await runLfwork(["claim", itemId])
    }

    func release(_ itemId: String) async throws {
        _ = try await runLfwork(["release", itemId])
    }

    func blockedItems() async throws -> [WorkItem] {
        let output = try await runLfwork(["blocked", "--verbose"])
        return parseListOutput(output)
    }

    func nextItem() async throws -> WorkItem? {
        let output = try await runLfwork(["next"])
        let items = parseListOutput(output)
        return items.first
    }

    // MARK: - Private

    private func runLfwork(_ args: [String]) async throws -> String {
        let process = Process()
        let pipe = Pipe()

        process.executableURL = URL(fileURLWithPath: "/usr/bin/env")
        process.arguments = ["lfwork"] + args
        process.currentDirectoryURL = URL(fileURLWithPath: repoPath)
        process.standardOutput = pipe
        process.standardError = pipe

        try process.run()
        process.waitUntilExit()

        let data = pipe.fileHandleForReading.readDataToEndOfFile()
        return String(data: data, encoding: .utf8) ?? ""
    }

    private func parseListOutput(_ output: String) -> [WorkItem] {
        var items: [WorkItem] = []

        let lines = output.split(separator: "\n", omittingEmptySubsequences: false)
        var currentItem: (id: String, title: String, status: WorkStatus, claimed: String?, blocked: String?, worktree: String?)?

        for line in lines {
            let trimmed = String(line).trimmingCharacters(in: .whitespaces)

            // Parse main item line: "✓ item-id              Title here [human] [blocked: reason]"
            if let firstChar = trimmed.first, ["?", "✓", "▶", "✔"].contains(String(firstChar)) {
                // Save previous item if any
                if let prev = currentItem {
                    items.append(WorkItem(
                        id: prev.id,
                        title: prev.title,
                        status: prev.status,
                        claimedBy: prev.claimed,
                        blockedOn: prev.blocked,
                        worktree: prev.worktree
                    ))
                }

                let status: WorkStatus
                switch String(firstChar) {
                case "?": status = .proposed
                case "✓": status = .approved
                case "▶": status = .active
                case "✔": status = .done
                default: status = .proposed
                }

                let rest = trimmed.dropFirst().trimmingCharacters(in: .whitespaces)
                let parts = rest.split(separator: " ", maxSplits: 1)
                guard parts.count >= 1 else { continue }

                let id = String(parts[0])
                var title = parts.count > 1 ? String(parts[1]) : id

                // Extract [human] and [blocked: ...] markers
                var claimed: String? = nil
                var blocked: String? = nil

                if title.contains("[human]") {
                    claimed = "human"
                    title = title.replacingOccurrences(of: "[human]", with: "").trimmingCharacters(in: .whitespaces)
                }

                if let blockedRange = title.range(of: "\\[blocked: [^\\]]+\\]", options: .regularExpression) {
                    let blockedMatch = String(title[blockedRange])
                    blocked = String(blockedMatch.dropFirst(10).dropLast())
                    title = title.replacingCharacters(in: blockedRange, with: "").trimmingCharacters(in: .whitespaces)
                }

                currentItem = (id: id, title: title, status: status, claimed: claimed, blocked: blocked, worktree: nil)
            }
            // Parse detail line: "    status=approved worktree=feature-x"
            else if trimmed.hasPrefix("status="), var item = currentItem {
                let parts = trimmed.split(separator: " ")
                for part in parts {
                    if part.hasPrefix("worktree=") {
                        item.worktree = String(part.dropFirst(9))
                    }
                }
                currentItem = item
            }
        }

        // Don't forget the last item
        if let prev = currentItem {
            items.append(WorkItem(
                id: prev.id,
                title: prev.title,
                status: prev.status,
                claimedBy: prev.claimed,
                blockedOn: prev.blocked,
                worktree: prev.worktree
            ))
        }

        return items
    }
}
