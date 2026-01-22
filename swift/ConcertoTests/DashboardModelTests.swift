// Tests for dashboard-related models: FileDiffStat, CommitInfo.

import Foundation
import Testing
@testable import LoopflowCore

@Suite("FileDiffStat Model")
struct FileDiffStatTests {

    @Test("Extracts filename from path")
    func filenameExtraction() {
        let stat = FileDiffStat(
            id: "src/views/Dashboard.swift",
            path: "src/views/Dashboard.swift",
            additions: 50,
            deletions: 10
        )

        #expect(stat.filename == "Dashboard.swift")
    }

    @Test("Extracts directory from path")
    func directoryExtraction() {
        let stat = FileDiffStat(
            id: "src/views/Dashboard.swift",
            path: "src/views/Dashboard.swift",
            additions: 50,
            deletions: 10
        )

        #expect(stat.directory == "src/views/")
    }

    @Test("Directory is empty for root-level files")
    func rootLevelFile() {
        let stat = FileDiffStat(
            id: "README.md",
            path: "README.md",
            additions: 5,
            deletions: 2
        )

        #expect(stat.filename == "README.md")
        #expect(stat.directory == "")
    }

    @Test("Calculates total changes")
    func totalChanges() {
        let stat = FileDiffStat(
            id: "file.swift",
            path: "file.swift",
            additions: 100,
            deletions: 25
        )

        #expect(stat.totalChanges == 125)
    }

    @Test("Extracts file extension")
    func fileExtension() {
        let swiftFile = FileDiffStat(id: "a.swift", path: "a.swift", additions: 1, deletions: 0)
        #expect(swiftFile.fileExtension == "swift")

        let tsxFile = FileDiffStat(id: "b.tsx", path: "b.tsx", additions: 1, deletions: 0)
        #expect(tsxFile.fileExtension == "tsx")

        let noExtension = FileDiffStat(id: "Makefile", path: "Makefile", additions: 1, deletions: 0)
        #expect(noExtension.fileExtension == "")
    }

    @Test("FileDiffStat is Equatable")
    func equatable() {
        let stat1 = FileDiffStat(id: "a.swift", path: "a.swift", additions: 10, deletions: 5)
        let stat2 = FileDiffStat(id: "a.swift", path: "a.swift", additions: 10, deletions: 5)
        let stat3 = FileDiffStat(id: "b.swift", path: "b.swift", additions: 10, deletions: 5)

        #expect(stat1 == stat2)
        #expect(stat1 != stat3)
    }
}

@Suite("CommitInfo Model")
struct CommitInfoTests {

    @Test("Stores commit data correctly")
    func storesData() {
        let date = Date()
        let commit = CommitInfo(
            id: "abc123def456",
            shortSHA: "abc123d",
            message: "Add new feature",
            author: "Test User",
            date: date
        )

        #expect(commit.id == "abc123def456")
        #expect(commit.shortSHA == "abc123d")
        #expect(commit.message == "Add new feature")
        #expect(commit.author == "Test User")
        #expect(commit.date == date)
    }

    @Test("Relative time formatting works")
    func relativeTime() {
        let recentDate = Date().addingTimeInterval(-60) // 1 minute ago
        let commit = CommitInfo(
            id: "abc123",
            shortSHA: "abc",
            message: "Test",
            author: "User",
            date: recentDate
        )

        // RelativeDateTimeFormatter returns localized strings, just verify it's not empty
        #expect(!commit.relativeTime.isEmpty)
    }

    @Test("CommitInfo is Identifiable by full SHA")
    func identifiable() {
        let commit = CommitInfo(
            id: "full-sha-hash",
            shortSHA: "short",
            message: "Test",
            author: "User",
            date: Date()
        )

        #expect(commit.id == "full-sha-hash")
    }

    @Test("CommitInfo is Equatable")
    func equatable() {
        let date = Date()
        let commit1 = CommitInfo(id: "abc", shortSHA: "a", message: "Test", author: "User", date: date)
        let commit2 = CommitInfo(id: "abc", shortSHA: "a", message: "Test", author: "User", date: date)
        let commit3 = CommitInfo(id: "xyz", shortSHA: "x", message: "Test", author: "User", date: date)

        #expect(commit1 == commit2)
        #expect(commit1 != commit3)
    }
}
