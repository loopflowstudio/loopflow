#if os(macOS)
import Foundation
import Testing
@testable import LoopflowMac

@Suite("Changes review stream")
struct ReviewDiffTests {
    /// Two files: an edited line and a moved block in `a.swift`, a new file.
    static let patch = """
    diff --git a/a.swift b/a.swift
    index 1111111..2222222 100644
    --- a/a.swift
    +++ b/a.swift
    @@ -1,9 +1,9 @@
    -func first() {
    -    let value = compute(1)
    -    return value
    -}
     let pageSize = 100
    -let label = "files"
    +let label = "changed files"
     struct Kept {}
    +func first() {
    +    let value = compute(2)
    +    return value
    +}
    diff --git a/b.swift b/b.swift
    new file mode 100644
    index 0000000..3333333
    --- /dev/null
    +++ b/b.swift
    @@ -0,0 +1,2 @@
    +let b = 1
    +let c = 2
    diff --git a/old.png b/new.png
    similarity index 90%
    rename from old.png
    rename to new.png
    Binary files a/old.png and b/new.png differ

    """

    @Test func streamReadsEveryFileWithStatusAndCounts() {
        let review = ReviewDiff.parse(Self.patch)
        #expect(review.files.map(\.path) == ["a.swift", "b.swift", "new.png"])
        #expect(review.files.map(\.status) == [.modified, .added, .binary])
        #expect(review.files[2].oldPath == "old.png")
        #expect(review.files[0].additions == 5 && review.files[0].deletions == 5)
        #expect(review.files[1].additions == 2 && review.files[1].deletions == 0)
    }

    @Test func nextAndPreviousCrossFileBoundaries() throws {
        let review = ReviewDiff.parse(Self.patch)
        let stops = review.stops
        #expect(stops.map(\.path) == ["a.swift", "a.swift", "a.swift", "b.swift"])
        var current: String?
        var visited: [String] = []
        while let next = review.stop(after: current, offset: 1) {
            visited.append(next.path)
            current = next.id
        }
        #expect(visited == stops.map(\.path))
        #expect(review.stop(after: stops[3].id, offset: -1)?.id == stops[2].id)
        #expect(review.stop(after: nil, offset: -1)?.path == "b.swift")
    }

    @Test func modifiedLineEmphasizesOnlyTheChangedWords() throws {
        let (old, new) = try #require(ReviewSegment.edit(#"let label = "files""#, #"let label = "changed files""#))
        #expect(old.filter(\.changed).map(\.text).joined().isEmpty)
        #expect(new.filter(\.changed).map(\.text).joined() == "changed ")
        #expect(new.map(\.text).joined() == #"let label = "changed files""#)
        // Unrelated lines read as a removal and an addition, not a word soup.
        #expect(ReviewSegment.edit("return value", "struct Kept {}") == nil)
    }

    @Test func movedBlockCollapsesToOneRowWithItsEdits() throws {
        let file = try #require(ReviewDiff.parse(Self.patch).file("a.swift"))
        let moves = file.blocks.compactMap { block -> ReviewMove? in
            if case .moved(let move) = block { return move }
            return nil
        }
        #expect(moves.map(\.end) == [.source, .destination])
        let destination = try #require(moves.last)
        #expect(destination.from == 1 && destination.to == 4)
        #expect(destination.lines.count == 4 && destination.changedLines == 1)
        let edited = try #require(destination.pairs.first { $0.removed?.text != $0.added?.text })
        #expect(edited.addedSegments?.filter(\.changed).map(\.text) == ["2"])
        // The label edit stays an ordinary change between them.
        #expect(file.blocks.contains { if case .change = $0 { true } else { false } })
    }

    @Test func shortRunsAreNotMoves() {
        let patch = """
        diff --git a/c.swift b/c.swift
        --- a/c.swift
        +++ b/c.swift
        @@ -1,4 +1,4 @@
        -}
         a
         b
        +}
        """
        let file = ReviewDiff.parse(patch).files[0]
        #expect(!file.blocks.contains { if case .moved = $0 { true } else { false } })
    }

    @Test func selectionNamesFileSideRangeAndRevisionOnOneLine() throws {
        let review = ReviewDiff.parse(Self.patch)
        let selection = ReviewSelection(path: "a.swift", side: .new, first: 6, last: 6).extended(to: 4)
        let reference = selection.reference(revision: "working tree on abc1234")
        #expect(reference == "`a.swift:L4-6` (after, working tree on abc1234) ")
        #expect(!reference.contains("\n"))
        #expect(selection.text(in: review) == "func first() {\n    let value = compute(2)\n    return value")
    }
}
#endif
