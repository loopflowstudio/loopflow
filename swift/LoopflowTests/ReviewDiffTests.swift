#if os(macOS)
import Foundation
@testable import Loopflow
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

    /// The patch `task_changes_count_each_files_stops_before_any_patch_is_read`
    /// counts in Rust: the title's total and the stepper walk the same stops.
    @Test func stopsMatchTheCountsTheServerSends() throws {
        let fixture = URL(fileURLWithPath: #filePath).deletingLastPathComponent().deletingLastPathComponent()
            .deletingLastPathComponent().appendingPathComponent("tests/fixtures/review/change_counts.patch")
        let review = ReviewDiff.parse(try String(contentsOf: fixture, encoding: .utf8))
        let counts = Dictionary(grouping: review.stops, by: \.path).mapValues(\.count)
        #expect(counts == ["edited.txt": 2, "removed.txt": 1, "with space.txt": 1, "new.txt": 1])
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
        #expect(selection.text(in: try #require(review.file("a.swift"))) == "func first() {\n    let value = compute(2)\n    return value")
    }

    @Test func hunkHeadersReadAsSkippedLinesAndTheirDeclaration() {
        #expect(ReviewGap("@@ -86,5 +90,5 @@ final class Store {", after: 40).label == "49 unchanged lines · final class Store {")
        #expect(ReviewGap("@@ -2,3 +2,4 @@", after: 0).label == "1 unchanged line")
        // A hunk that starts the file, or follows the last line shown, skips nothing.
        #expect(ReviewGap("@@ -1,3 +1,4 @@", after: 0).label == "")
        // A hunk that only removes names the line before it: line 4 is skipped too.
        #expect(ReviewGap("@@ -5,2 +4,0 @@", after: 0).new == 1..<5)
        // Each hunk counts from the last new line shown above it.
        let gaps = ReviewDiff.parse(Self.patch).files[0].gaps
        #expect(gaps.count == ReviewDiff.parse(Self.patch).files[0].blocks.filter { if case .hunk = $0 { true } else { false } }.count)
    }

    /// Two hunks of `a.swift` with new lines 3–7 skipped between them; one line
    /// was added above, so old numbers trail new ones by one.
    static let gapped = """
    diff --git a/a.swift b/a.swift
    --- a/a.swift
    +++ b/a.swift
    @@ -1,1 +1,2 @@
     one
    +two
    @@ -7,2 +8,2 @@ struct Kept {
     eight
    -nine
    +nueve

    """

    @Test func skippedLinesOpenInPlaceFromTheFile() throws {
        let file = ReviewDiff.parse(Self.gapped).files[0]
        let (index, header) = try #require(file.blocks.enumerated().compactMap { index, block -> (Int, String)? in
            if case .hunk(let header) = block, index > 0 { return (index, header) }
            return nil
        }.first)
        let gap = try #require(file.gaps[index])
        #expect(gap.label == "5 unchanged lines · struct Kept {")
        let content = "one\ntwo\nthree\nfour\nfive\nsix\nseven\neight\nnueve\n"
        let lines = try #require(gap.lines(in: content))
        #expect(lines.map(\.text) == ["three", "four", "five", "six", "seven"])
        #expect(lines.map(\.new) == [3, 4, 5, 6, 7] && lines.map(\.old) == [2, 3, 4, 5, 6])
        // The header gives way to its lines: one run from line 1 to the change, same changes.
        let opened = file.opening([header: lines])
        #expect(opened.lines.filter { $0.kind != .removed }.compactMap(\.new) == Array(1...9))
        #expect(!opened.blocks.dropFirst().contains { if case .hunk = $0 { true } else { false } })
        #expect(ReviewDiff(files: [opened]).stops == ReviewDiff(files: [file]).stops)
        // A file too short to hold the skipped lines is not the file this hunk describes.
        #expect(gap.lines(in: "one\ntwo\n") == nil)
    }

    @Test func commentsFollowTheirLinesAndGoToTheConversationAsOneLine() throws {
        let file = ReviewDiff.parse(Self.gapped).files[0]
        let kept = ReviewComment(selection: ReviewSelection(path: "a.swift", side: .new, first: 8, last: 9),
                                 text: "why Spanish?\nsay so", in: file)
        let gone = ReviewComment(selection: ReviewSelection(path: "a.swift", side: .old, first: 8, last: 8),
                                 text: "keep this", in: file)
        #expect(kept.anchor == ["eight", "nueve"] && gone.anchor == ["nine"])
        // The worker adds three lines above and stops removing `nine`.
        let edited = ReviewDiff.parse("""
        diff --git a/a.swift b/a.swift
        --- a/a.swift
        +++ b/a.swift
        @@ -1,1 +1,5 @@
         one
        +two
        +a
        +b
        +c
        @@ -7,2 +11,2 @@ struct Kept {
         eight
        -neuf
        +nueve

        """).files[0]
        let moved = kept.anchored(in: edited)
        #expect(moved.selection.range == "L11-12" && !moved.detached)
        let detached = gone.anchored(in: edited)
        #expect(detached.detached && detached.selection.range == "L8")
        // A file that left the comparison detaches its comments; none is dropped.
        #expect(kept.anchored(in: nil).detached)
        let message = ReviewComment.message([moved, detached], revision: "Compared with main at abc1234. Includes uncommitted changes")
        #expect(message == "2 comments on the diff (Compared with main at abc1234. Includes uncommitted changes): "
            + "(1) `a.swift:L11-12` (after): why Spanish? say so "
            + "(2) `a.swift:L8` (before, lines since changed): keep this ")
        #expect(!message.unicodeScalars.contains { $0.value < 0x20 })
        #expect(ReviewComment.message([moved], revision: "abc") == "`a.swift:L11-12` (after): why Spanish? say so (abc) ")
    }

    @Test func aTourPutsItsFilesFirstAndTheRestByGroup() throws {
        let tour = DiffTour.parse("""
        # written when the change is handed over
        at: 81d3fa5973e63a4394a25357502e635a3baf5d5b
        first:
          - path: swift/Store.swift
            why: "A change over 1 MB no longer loses its last files"
          - swift/StoreTests.swift
          - path: ./gone.swift
            why: Not in this change
        hide:
          - path: docs/
            why: Path literal renamed, nothing else
        other:
          - path: ignored
        """)
        #expect(tour.at == "81d3fa5973e63a4394a25357502e635a3baf5d5b")
        #expect(DiffTour.parse("first:\n  - a.swift\n").at == nil)
        #expect(tour.first.map(\.path) == ["swift/Store.swift", "swift/StoreTests.swift", "gone.swift"])
        #expect(tour.hide == [DiffTour.Entry(path: "docs/", why: "Path literal renamed, nothing else")])
        #expect(tour.hide[0].matches("docs/config.md") && !tour.hide[0].matches("src/docs.rs"))

        func file(_ path: String, lines: Int) -> ReviewFile {
            let added = (1...lines).map { ReviewLine(kind: .added, text: "x", old: nil, new: $0) }
            return ReviewFile(path: path, oldPath: nil, status: .modified,
                              blocks: [.change(ReviewChange(id: "\(path)#n1", removed: [], added: added))])
        }
        let files = [file("Cargo.lock", lines: 90), file("README.md", lines: 5), file("rust/a.rs", lines: 2),
                     file("rust/b.rs", lines: 40), file("rust/tests/a.rs", lines: 60), file("swift/Store.swift", lines: 1),
                     file("swift/StoreTests.swift", lines: 3)]
        let groups = try #require(DiffGroupSetting.parse(.array([
            .object(["title": .string("Docs"), "paths": .array([.string("*.md")])]),
            .object(["rest": .null]),
            .object(["title": .string("Tests"), "paths": .array([.string("/tests/"), .string("tests.")])]),
            .object(["title": .string("Generated"), "paths": .array([.string("*.lock")])]),
        ])))
        #expect(groups[1] == DiffGroupSetting(title: "Everything else", paths: nil))
        #expect(DiffGroupSetting.parse(.array([.object(["title": .string("No paths")])])) == nil)
        let byGroup = ReviewGroup.arrange(files, ordering: .groups, tour: tour, groups: groups)
        #expect(byGroup.map(\.title) == ["Docs", "Everything else", "Tests", "Generated"])
        // By path within a group.
        #expect(byGroup.flatMap(\.files).map(\.path) == ["README.md", "rust/a.rs", "rust/b.rs", "swift/Store.swift",
                                                          "rust/tests/a.rs", "swift/StoreTests.swift", "Cargo.lock"])
        // A list without a rest puts unmatched files last.
        #expect(ReviewGroup.arrange(files, ordering: .groups, tour: nil, groups: [groups[3]]).map(\.title)
                == ["Generated", "Everything else"])
        let toured = ReviewGroup.arrange(files, ordering: .tour, tour: tour, groups: groups)
        // An entry without a reason reads on under the one above; unnamed files follow by group.
        #expect(toured.map(\.title) == ["A change over 1 MB no longer loses its last files", "Docs", "Everything else",
                                       "Tests", "Generated"])
        #expect(toured[0].isTour && toured[0].files.map(\.path) == ["swift/Store.swift", "swift/StoreTests.swift"])
        #expect(Set(toured.flatMap(\.files).map(\.path)) == Set(files.map(\.path)))
        let paths = ReviewGroup.arrange(files, ordering: .path, tour: tour, groups: groups)
        #expect(paths.map(\.title) == ["Top level", "rust/", "swift/"])
        // No tour: the tour order is the group order.
        #expect(ReviewGroup.arrange(files, ordering: .tour, tour: nil, groups: groups) == byGroup)
    }

    /// The bar under the file's name reads the declaration the top line is
    /// in: the nearest line above that opens one, else the hunk's.
    @Test func theEnclosingDeclarationIsTheNearestAbove() throws {
        let patch = """
        diff --git a/s.rs b/s.rs
        --- a/s.rs
        +++ b/s.rs
        @@ -10,6 +10,6 @@ impl Store {
             let a = 1;
        -    let b = 2;
        +    let b = 3;
         pub(crate) async fn read(&self) -> u8 {
             let c = 4;
        -    c
        +    c + 1
        """
        let file = try #require(ReviewDiff.parse(patch).files.first)
        let stops = file.stops
        #expect(file.enclosing(stops[0]) == "impl Store {")
        #expect(file.enclosing(stops[1]) == "pub(crate) async fn read(&self) -> u8 {")
        #expect(ReviewLine(kind: .context, text: "## Remaining", old: 1, new: 1).declaration(markdown: true) == "## Remaining")
        #expect(ReviewLine(kind: .context, text: "# a comment", old: 1, new: 1).declaration(markdown: false) == nil)
        #expect(ReviewLine(kind: .context, text: "let function = 1", old: 1, new: 1).declaration(markdown: false) == nil)
    }

    /// Commits whose subjects share `lf <step>:` in a run read as one step;
    /// any other commit stands alone, and each step compares against its
    /// first commit's parent. A merge between two runs, which the list never
    /// holds, keeps them two steps.
    @Test func commitsReadAsGraphSteps() {
        let commits = [("1", "base", "lf implement: a"), ("2", "1", "lf implement: b"), ("3", "2", "fix a typo"),
                       ("4", "3", "lf implement: c"), ("6", "merge", "lf implement: d"), ("7", "6", "lf gate: e")]
            .map { TaskCommit(sha: $0.0, parent: $0.1, subject: $0.2) }
        let steps = ReviewStep.steps(commits)
        #expect(steps.map(\.title) == ["implement · 2 commits", "fix a typo", "implement", "implement", "gate"])
        #expect(steps.map(\.base) == ["base", "2", "3", "merge", "6"])
        #expect(steps.map(\.head) == ["2", "3", "4", "6", "7"])
    }

    /// A file stays viewed only while its lines read the same.
    @Test func aFilesFingerprintFollowsItsLines() {
        let one = ReviewDiff.parse(Self.patch).files[1]
        let again = ReviewDiff.parse(Self.patch).files[1]
        let edited = ReviewDiff.parse(Self.patch.replacingOccurrences(of: "+let c = 2", with: "+let c = 3")).files[1]
        #expect(one.fingerprint == again.fingerprint && one.fingerprint != edited.fingerprint)
    }

    /// A line's comments, strings, numbers and keywords are found by the
    /// file's language; a block comment carries to the next line, and a file
    /// of an unknown kind gets no color.
    @Test func syntaxColorsFollowTheFilesLanguage() throws {
        func kinds(_ line: String, in path: String, inBlock: inout Bool) -> [String] {
            let language = SyntaxLanguage.named(path)!
            return language.spans(line, inBlock: &inBlock).map { span in
                "\(span.kind):\((line as NSString).substring(with: span.range))"
            }
        }
        var inBlock = false
        #expect(kinds(#"let count = 42 // "not a string""#, in: "a/Store.swift", inBlock: &inBlock)
                == ["keyword:let", "number:42", #"comment:// "not a string""#])
        #expect(kinds(#"print("a \" // b", lets)"#, in: "Store.swift", inBlock: &inBlock) == [#"string:"a \" // b""#])
        #expect(kinds("fn main() { /* open", in: "main.rs", inBlock: &inBlock) == ["keyword:fn", "comment:/* open"])
        #expect(inBlock)
        #expect(kinds("still */ let x: &'a str = 0;", in: "main.rs", inBlock: &inBlock)
                == ["comment:still */", "keyword:let", "number:0"])
        #expect(!inBlock)
        #expect(kinds("echo $# # count", in: "run.sh", inBlock: &inBlock) == ["comment:# count"])
        #expect(kinds("enabled: true # yes", in: ".lf/config.yaml", inBlock: &inBlock) == ["keyword:true", "comment:# yes"])
        #expect(SyntaxLanguage.named("notes.md") == nil && SyntaxLanguage.named("Dockerfile") != nil)
    }
}
#endif
