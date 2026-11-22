#if os(macOS)
import Foundation
@testable import Loopflow
import Testing
@testable import LoopflowMac

@MainActor
@Suite("Markdown redline")
struct MarkdownRedlineTests {
    /// Each entry as a reader sees it: what changed and the new (or old) text.
    static func read(_ redline: MarkdownRedline) -> [String] {
        redline.entries.map { entry in
            switch entry {
            case .same(let block): "= \(String(block.text.characters))"
            case .removed(let block): "- \(String(block.text.characters))"
            case .added(let block): "+ \(String(block.text.characters))"
            case .changed(_, let new, _): "~ \(String(new.text.characters))"
            }
        }
    }

    static func words(_ entry: MarkdownRedline.Entry, _ kind: ReviewWords.Kind) -> [String] {
        guard case .changed(_, _, let words) = entry else { return [] }
        return words.filter { $0.kind == kind }.map { $0.text.trimmingCharacters(in: .whitespaces) }
    }

    @Test func anEditedHeadingIsOneChangedBlockWithItsNewWordMarked() throws {
        let redline = MarkdownRedline(old: "# Diff plan\n\nBody stays.\n", new: "# Diff v2 plan\n\nBody stays.\n")
        #expect(Self.read(redline) == ["~ Diff v2 plan", "= Body stays."])
        let heading = try #require(redline.entries.first)
        #expect(Self.words(heading, .added) == ["v2"])
        #expect(Self.words(heading, .removed).isEmpty)
        #expect(heading.old?.kind == .heading(1) && heading.new?.kind == .heading(1))
    }

    @Test func anInsertedParagraphStandsAloneBetweenItsNeighbours() {
        let redline = MarkdownRedline(old: "First.\n\nLast.\n", new: "First.\n\nA new thought.\n\nLast.\n")
        #expect(Self.read(redline) == ["= First.", "+ A new thought.", "= Last."])
    }

    @Test func aDeletedListItemIsStruckWholeAndTheOthersAnchorDespiteRenumbering() {
        let redline = MarkdownRedline(old: "1. Read\n2. Pair\n3. Render\n", new: "1. Read\n2. Render\n")
        #expect(Self.read(redline) == ["= Read", "- Pair", "= Render"])
        // The surviving item carries its new number.
        #expect(redline.entries.last?.new?.marker == "2.")
    }

    @Test func aRewriteSharingTooFewWordsIsStruckAndFilledNotPaired() {
        let redline = MarkdownRedline(
            old: "Intro.\n\nThe grammars ship inside the app bundle.\n",
            new: "Intro.\n\nEvery theme passes contrast checks now.\n")
        #expect(Self.read(redline) == ["= Intro.", "- The grammars ship inside the app bundle.",
                                       "+ Every theme passes contrast checks now."])
    }

    @Test func changedWordsKeepTheirSideInlineStyleInEachColumnAndTheRedline() throws {
        let redline = MarkdownRedline(old: "Run the **old** check.\n", new: "Run the **new** check.\n")
        let entry = try #require(redline.entries.first)
        guard case .changed(let old, let new, let words) = entry else {
            Issue.record("expected one changed paragraph, got \(Self.read(redline))")
            return
        }
        let line = MarkdownRedline.pieces(words, old: old.text, new: new.text, showing: [.same, .removed, .added])
        #expect(line.map { String($0.text.characters) }.joined() == "Run the oldnew check.")
        let right = MarkdownRedline.pieces(words, old: old.text, new: new.text, showing: [.same, .added])
        #expect(right.map { String($0.text.characters) }.joined() == "Run the new check.")
        let added = try #require(line.first { $0.kind == .added })
        #expect(added.text.runs.first?.inlinePresentationIntent == .stronglyEmphasized)
    }

    @Test func anAddedDocumentIsEveryBlockAdded() {
        #expect(Self.read(MarkdownRedline(old: "", new: "# Plan\n\nStart.\n")) == ["+ Plan", "+ Start."])
    }
}
#endif
