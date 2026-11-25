#if os(macOS)
import AppKit
import Loopflow
import SwiftUI

/// The one diff reader. Changes shows the whole comparison through it; Files'
/// Diff mode shows one file, draft included. Rows, layout, word emphasis,
/// moves, skipped lines that open in place, line selection and the comments
/// left under lines are the same in both.
struct ReviewStream: View {
    @Bindable var store: TaskFilesStore
    let reader: DiffReviewState.ReviewReader
    let files: [ReviewFile]
    /// Side by side; callers fall back to unified in a narrow pane.
    let split: Bool
    /// Words to read above a file, by its path.
    var headings: [String: String] = [:]
    /// False when the caller names the first file itself, above the stream.
    var headsFirstFile = true
    let addToConversation: ((String) -> Bool)?
    @Environment(\.palette) private var palette
    @Environment(\.theme) private var theme

    private var reading: ComparisonReading { store.diffReview.reading(for: reader) }
    private var revision: String { store.diffReview.revision(for: reader) }

    private var presentedLines: ClosedRange<Int>? {
        guard case .file(let path) = reader, store.selection == path else { return nil }
        return store.visit?.lines
    }

    var body: some View {
        if reader == .changes {
            // A comment whose file left the comparison still waits to be sent.
            ForEach(store.comments.items.filter { comment in
                guard let selection = comment.selection, let original = comment.original else { return false }
                return original.source.revision == revision && store.diffReview.review?.file(selection.path) == nil
            }) { comment in
                ReviewCommentRow(store: store, comment: comment)
            }
        }
        ForEach(files) { unopened in
            let file = store.diffReview.opened(unopened, in: reader)
            // Reading state and notes belong to the whole comparison's per-file reads.
            let note = reader == .changes ? store.diffReview.fileNotes[file.path] : nil
            if let heading = headings[file.path] {
                Text(heading).font(Typography.textStrong).foregroundStyle(palette.text)
                    .fixedSize(horizontal: false, vertical: true)
                    .padding(.horizontal, Spacing.md).padding(.top, Spacing.xl)
                    .accessibilityAddTraits(.isHeader)
            }
            if headsFirstFile || file.path != files.first?.path {
                fileHeader(file, headed: headings[file.path] != nil).id("file:\(file.path)")
            }
            ForEach(store.comments.items.filter { $0.detached && $0.original?.source.revision == revision && $0.selection?.path == file.path }) { comment in
                ReviewCommentRow(store: store, comment: comment)
            }
            if reader == .changes, store.diffReview.isViewed(file) {
                // Viewed: folded to its header until you clear it or its lines change.
            } else if reader == .changes, store.diffReview.isReading(file) {
                ReviewNotice(text: "Reading…").id(store.diffReview.topID(of: file, in: reader))
                    .onAppear { store.diffReview.setNear(file.path, true) }
                    .onDisappear { store.diffReview.setNear(file.path, false) }
            } else {
                if let rendering = store.diffReview.rendering(of: file, in: reader) {
                    if let error = reading.sideErrors[file.path] {
                        ReviewSidesFailed(store: store, reader: reader, file: file, error: error).id(ReviewStop.rendered(file.path).id)
                        // A file with Source falls back to its line Diff.
                        if rendering.hasSource { lines(file, note: note) }
                    } else {
                        ReviewRendered(store: store, reader: reader, file: file, rendering: rendering, split: split)
                            .id(ReviewStop.rendered(file.path).id)
                    }
                } else {
                    lines(file, note: note)
                }
            }
            if let note { ReviewFileNote(note: note) { Task { await store.diffReview.retryFile(file.path) } } }
        }
    }

    /// The file's line Diff: its hunks, or what stands in when it has none.
    @ViewBuilder private func lines(_ file: ReviewFile, note: DiffReviewState.FileNote?) -> some View {
        if let machine = reading.openedGaps[file.path]?.failure {
            FailureLine("Skipped lines did not load.", machine: machine).reviewNoticeInset()
        }
        if file.status == .binary {
            ReviewNotice(text: "Binary file. No text to compare.").id(file.topID)
        } else if file.blocks.isEmpty, note == nil {
            ReviewNotice(text: file.status == .renamed ? "Renamed without changes." : "No textual changes.")
                .id(file.topID)
        }
        let gaps = file.gaps
        ForEach(Array(file.blocks.enumerated()), id: \.offset) { index, block in
            blockView(block, gap: gaps[index], file: file)
        }
    }

    static let gutterWidth: CGFloat = 36

    /// Where new code dropped lines: how many, and the way to read them.
    private func removalMark(_ count: Int, change: String) -> some View {
        Button { store.diffReview.openedRemovals.insert(change) } label: {
            HStack(spacing: Spacing.xs) {
                Rectangle().fill(palette.failureTone.ink).frame(width: 2, height: 12)
                Text(count == 1 ? "1 old line · Show" : "\(count) old lines · Show")
            }
            .font(Typography.meta).foregroundStyle(palette.failureTone.ink)
            .padding(.leading, Self.gutterWidth + 6).padding(.vertical, Spacing.xxs)
            .frame(maxWidth: .infinity, alignment: .leading)
            .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .help("Show the lines this change removed")
        .accessibilityIdentifier("changes-old-lines")
    }

    private func fileHeader(_ file: ReviewFile, headed: Bool) -> some View {
        ReviewFileTitle(store: store, file: file, note: file.oldPath.map { "from \($0)" }, viewable: reader == .changes)
            .padding(.vertical, Spacing.sm)
            .background(palette.surfaceMuted)
            .overlay(alignment: .top) { Divider() }
            .padding(.top, headed ? 6 : 16)
            .contextMenu {
                Button("Open in Files") { store.selectFile(file.path, mode: .diff) }
                Button("Read file") { store.selectFile(file.path, mode: .read) }
            }
    }

    /// A block's rows, each with its id. A change gives one lazy row per
    /// line, so a long change is drawn only as it scrolls into view.
    @ViewBuilder
    private func blockView(_ block: ReviewBlock, gap: ReviewGap?, file: ReviewFile) -> some View {
        switch block {
        case .hunk(let header):
            gapRow(header, gap: gap, file: file).id(block.id(in: file.path))
        case .context(let line):
            pairRow(ReviewPair(removed: line, added: line, removedSegments: nil, addedSegments: nil),
                    file: file, context: true)
                .id(block.id(in: file.path))
        case .change(let change):
            ForEach(Array(rows(of: change, in: file).enumerated()), id: \.offset) { offset, row in
                changeRow(row, change: change, file: file)
                    .overlay { currentMark(change.id) }
                    .id(change.rowID(offset))
            }
        case .moved(let move):
            movedView(move, file: file)
                .overlay { currentMark(move.id) }
                .id(block.id(in: file.path))
        }
    }

    private enum ChangeRow {
        /// New code's fold of the removed lines, with how many there are.
        case removals(Int)
        case line(ReviewLine, [ReviewSegment]?)
        case pair(ReviewPair)
    }

    /// Side by side, one row per pair. Unified, every removed line, then every
    /// added line; new code folds the removed lines to a mark that opens them.
    /// A new file is its contents.
    private func rows(of change: ReviewChange, in file: ReviewFile) -> [ChangeRow] {
        if file.status == .added { return change.added.map { .line($0, nil) } }
        if split { return change.pairs.map(ChangeRow.pair) }
        var rows: [ChangeRow] = []
        if store.diffReview.reviewLayout == .new, !store.diffReview.openedRemovals.contains(change.id) {
            if !change.removed.isEmpty { rows.append(.removals(change.removed.count)) }
        } else {
            rows += change.pairs.compactMap { pair in pair.removed.map { .line($0, pair.removedSegments) } }
        }
        rows += change.pairs.compactMap { pair in pair.added.map { .line($0, pair.addedSegments) } }
        return rows
    }

    /// Use the same rows and folds as the stream, including both columns in Split.
    func searchCells(in file: ReviewFile) -> [(FileFind.Cell, String)] {
        var cells: [(FileFind.Cell, String)] = []
        func append(_ line: ReviewLine?, _ side: ReviewSelection.Side, row: String) {
            guard let line, let number = side == .old ? line.old : line.new else { return }
            cells.append((.diff(row: row, side: side, line: number), visibleText(line, path: file.path)))
        }
        func pair(_ pair: ReviewPair, row: String) {
            append(pair.removed, .old, row: row)
            append(pair.added, .new, row: row)
        }
        for block in file.blocks {
            let row = block.id(in: file.path)
            switch block {
            case .context(let line):
                if split && file.status != .added { append(line, .old, row: row) }
                append(line, .new, row: row)
            case .change(let change):
                for (offset, entry) in rows(of: change, in: file).enumerated() {
                    switch entry {
                    case .removals: break
                    case .line(let line, _): append(line, line.kind == .removed ? .old : .new, row: change.rowID(offset))
                    case .pair(let value): pair(value, row: change.rowID(offset))
                    }
                }
            case .moved(let move) where move.end == .destination && store.diffReview.expandedMoves.contains(move.id):
                for value in move.pairs { pair(value, row: row) }
            default: break
            }
        }
        return cells
    }

    private func visibleText(_ line: ReviewLine, path: String) -> String {
        store.diffReview.openedLongLines.contains(line.id(in: path)) ? line.text : line.cut?.shown ?? line.text
    }

    @ViewBuilder private func changeRow(_ row: ChangeRow, change: ReviewChange, file: ReviewFile) -> some View {
        switch row {
        case .removals(let count): removalMark(count, change: change.id)
        case .line(let line, let segments): lineRow(line, segments: segments, file: file)
        case .pair(let pair): pairRow(pair, file: file, context: false)
        }
    }

    /// The lines between two hunks: how many, and the declaration git names.
    /// A click reads them from the file and shows them in place.
    @ViewBuilder private func gapRow(_ header: String, gap: ReviewGap?, file: ReviewFile) -> some View {
        let label = gap?.label ?? ""
        let row = HStack(spacing: Spacing.sm) {
            if !label.isEmpty {
                Text(label).font(Typography.meta).foregroundStyle(palette.textTertiary)
                    .lineLimit(1).truncationMode(.tail)
            }
            Rectangle().fill(palette.border).frame(height: 1)
        }
        .padding(.leading, Self.gutterWidth + 6).padding(.trailing, Spacing.md).padding(.vertical, Spacing.xs)
        if let gap, !gap.new.isEmpty {
            Button {
                Task { await store.diffReview.openGap(header, gap: gap, file: file, reader: reader) }
            } label: {
                row.overlay(alignment: .leading) {
                    Image(systemName: "arrow.up.and.down").font(Typography.meta).fontWeight(.semibold)
                        .foregroundStyle(palette.textSecondary).frame(width: Self.gutterWidth, alignment: .trailing)
                }
                .contentShape(Rectangle())
            }
            .buttonStyle(.plain)
            .help(gap.new.count == 1 ? "Show the unchanged line" : "Show the \(gap.new.count) unchanged lines")
            .accessibilityLabel("Show \(label)")
        } else {
            row
        }
    }

    private func currentMark(_ id: String) -> some View {
        ReviewCurrentMark(store: store, reader: reader, id: id)
    }

    // MARK: Moved code

    private func movedView(_ move: ReviewMove, file: ReviewFile) -> some View {
        let expanded = store.diffReview.expandedMoves.contains(move.id)
        return VStack(alignment: .leading, spacing: 0) {
            Button {
                if move.end == .source {
                    store.diffReview[place: reader] = .init(anchor: move.counterpart, current: move.counterpart)
                } else if expanded {
                    store.diffReview.expandedMoves.remove(move.id)
                } else {
                    store.diffReview.expandedMoves.insert(move.id)
                }
            } label: {
                HStack(spacing: Spacing.xs) {
                    Image(systemName: move.end == .source ? "arrow.turn.right.down" : "arrow.left.arrow.right")
                        .frame(width: 14)
                    Text(Self.movedLabel(move))
                    Spacer(minLength: 0)
                    if move.end == .destination { Image(systemName: expanded ? "chevron.down" : "chevron.right") }
                }
                .font(Typography.text)
                .foregroundStyle(palette.text)
                .padding(.leading, Self.gutterWidth + 6).padding(.trailing, Spacing.md).padding(.vertical, Spacing.xs)
                .background(palette.surfaceElement)
                .contentShape(Rectangle())
            }
            .buttonStyle(.plain)
            .accessibilityLabel(Self.movedLabel(move))
            .accessibilityValue(move.end == .destination ? (expanded ? "Expanded" : "Collapsed") : "")
            if expanded, move.end == .destination {
                ForEach(Array(move.pairs.enumerated()), id: \.offset) { _, pair in
                    pairRow(pair, file: file, context: false)
                }
            }
        }
    }

    private static func movedLabel(_ move: ReviewMove) -> String {
        let changed = move.changedLines == 0 ? "unchanged" : "\(move.changedLines) lines changed"
        switch move.end {
        case .destination: return "Moved here from L\(move.from) · \(move.lines.count) lines, \(changed)"
        case .source: return "Moved to L\(move.to) · \(move.lines.count) lines"
        }
    }

    // MARK: Lines

    /// Unified: one line with one number, the old one on a removed line.
    private func lineRow(_ line: ReviewLine, segments: [ReviewSegment]?, file: ReviewFile) -> some View {
        let side: ReviewSelection.Side = line.kind == .removed ? .old : .new
        return VStack(spacing: 0) {
            HStack(spacing: 0) {
                gutter(side == .old ? line.old : line.new, line: line, side: side, file: file)
                cell(line, segments: segments, side: side, file: file)
            }
            attachments(old: line.kind == .added ? nil : line.old, new: line.kind == .removed ? nil : line.new, file: file)
        }
    }

    /// Side by side: before on the left, after on the right. Context repeats on
    /// both sides; a pure addition or removal leaves the other side empty.
    @ViewBuilder private func pairRow(_ pair: ReviewPair, file: ReviewFile, context: Bool) -> some View {
        if (split && file.status != .added) || !context {
            VStack(spacing: 0) {
                HStack(spacing: 0) {
                    HStack(spacing: 0) {
                        gutter(pair.removed?.old, line: pair.removed, side: .old, file: file)
                        if let line = pair.removed { cell(line, segments: pair.removedSegments, side: .old, file: file) }
                        else { empty }
                    }
                    Divider()
                    HStack(spacing: 0) {
                        gutter(pair.added?.new, line: pair.added, side: .new, file: file)
                        if let line = pair.added { cell(line, segments: pair.addedSegments, side: .new, file: file) }
                        else { empty }
                    }
                }
                attachments(old: pair.removed?.old, new: pair.added?.new, file: file)
            }
        } else if let line = pair.added {
            lineRow(line, segments: nil, file: file)
        }
    }

    /// What sits under a row: the comment being written when the selection
    /// ends on it, then the comments left there. `old` and `new` are the line
    /// numbers the row shows.
    @ViewBuilder private func attachments(old: Int?, new: Int?, file: ReviewFile) -> some View {
        if let selection = store.comments.selection, selection.path == file.path,
           case .diff(let side, let selectionRevision) = selection.source, selectionRevision == revision,
           selection.last == (side == .old ? old : new) {
            ReviewCommentEditor(store: store, selection: selection, excerpt: { selection.excerpt(in: file) }, addToConversation: addToConversation)
        }
        ForEach(store.comments.items.filter { comment in
            guard let selection = comment.selection, case .diff(let side, let commentRevision) = selection.source else { return false }
            return commentRevision == revision && !comment.detached && selection.path == file.path
                && selection.last == (side == .old ? old : new)
        }) { comment in
            ReviewCommentRow(store: store, comment: comment)
        }
    }

    private var empty: some View {
        palette.surfaceMuted.opacity(0.4).frame(maxWidth: .infinity, maxHeight: .infinity)
    }

    private func gutter(_ number: Int?, line: ReviewLine?, side: ReviewSelection.Side, file: ReviewFile) -> some View {
        ReviewGutter(number: number, selected: line.map { isSelected($0, side: side, path: file.path) } ?? false,
                     tone: line.flatMap { tone(of: $0, in: file) }) {
            if let number { select(number, side: side, path: file.path) }
        }
    }

    /// Added and removed lines carry their color across the gutter and the
    /// line; a new file reads as its contents, without the marks of a patch.
    private func tone(of line: ReviewLine, in file: ReviewFile) -> ThemeTone? {
        switch line.kind {
        case .added: file.status == .added ? nil : palette.successTone
        case .removed: palette.failureTone
        case .context: nil
        }
    }

    private func isSelected(_ line: ReviewLine, side: ReviewSelection.Side, path: String) -> Bool {
        if side == .new, let number = line.new, presentedLines?.contains(number) == true { return true }
        guard let selection = store.comments.selection, selection.path == path,
              selection.source == .diff(side: side, revision: revision),
              let number = side == .old ? line.old : line.new else { return false }
        return (selection.first...selection.last).contains(number)
    }

    private func cell(_ line: ReviewLine, segments: [ReviewSegment]?, side: ReviewSelection.Side, file: ReviewFile) -> some View {
        let path = file.path
        let tone = tone(of: line, in: file)
        let selected = isSelected(line, side: side, path: path)
        // A line past the readable length shows its start until asked for the rest.
        let cut = line.cut.flatMap { store.diffReview.openedLongLines.contains(line.id(in: path)) ? nil : $0 }
        let text = cut?.shown ?? line.text
        let colored = Self.attributed(text, segments: cut == nil ? segments : nil, tone: tone,
                                      highlights: store.diffReview.highlights(of: file, in: reader)?.spans(for: line) ?? [],
                                      inks: SyntaxInks.of(theme))
        let find = reader == .changes ? nil : store.find
        let highlighted: AttributedString
        if let cell = find?.current?.cell, case .diff(_, let matchSide, let number) = cell,
           side == matchSide, number == (side == .old ? line.old : line.new) {
            highlighted = find?.highlight(colored, in: cell, color: palette.keyboardFocus.opacity(0.45)) ?? colored
        } else { highlighted = colored }
        return VStack(alignment: .leading, spacing: Spacing.xxs) {
            HStack(alignment: .top, spacing: Spacing.xs) {
                // A marker as well as color: + and − read without hue.
                // Selecting and copying takes the code only.
                Text(tone == nil ? " " : line.kind == .added ? "+" : "−")
                    .foregroundStyle(tone?.ink ?? palette.textSecondary)
                    .textSelection(.disabled)
                Text(highlighted)
                    .lineLimit(store.diffReview.wrapsLines ? nil : 1)
                    .frame(maxWidth: .infinity, alignment: .leading)
                    .fixedSize(horizontal: false, vertical: true)
                    .textSelection(.enabled)
            }
            .font(Typography.mono)
            .accessibilityElement(children: .combine)
            .accessibilityLabel("\(line.kind == .added ? "Added" : line.kind == .removed ? "Removed" : "Line") \(side == .old ? line.old ?? 0 : line.new ?? 0): \(text)")
            if let cut {
                Button("\(cut.more.formatted()) more characters · Show") { store.diffReview.openedLongLines.insert(line.id(in: path)) }
                    .buttonStyle(WorkPlainButtonStyle()).font(Typography.meta)
                    .padding(.leading, Spacing.md)
                    .accessibilityIdentifier("changes-long-line")
            }
        }
        .padding(.vertical, Spacing.xxs).padding(.trailing, Spacing.sm)
        .frame(maxWidth: .infinity, alignment: .leading)
        .background(selected ? palette.accent.opacity(0.16) : tone?.fill ?? .clear)
        .contextMenu { copyMenu(line, side: side, file: file) }
    }

    /// Copies code without the patch's + and − or the line numbers.
    @ViewBuilder private func copyMenu(_ line: ReviewLine, side: ReviewSelection.Side, file: ReviewFile) -> some View {
        Button("Copy line") { Self.copy(line.text) }
        if let selection = store.comments.selection, selection.path == file.path, selection.source == .diff(side: side, revision: revision),
           selection.first != selection.last, isSelected(line, side: side, path: file.path) {
            Button("Copy \(selection.last - selection.first + 1) selected lines") { Self.copy(selection.excerpt(in: file).joined(separator: "\n")) }
        }
        Button(side == .old ? "Copy the removed lines shown" : "Copy the new code shown") {
            Self.copy(ReviewSelection.side(side, of: file))
        }
    }

    static func copy(_ text: String) {
        NSPasteboard.general.clearContents()
        NSPasteboard.general.setString(text, forType: .string)
    }

    /// The line in its syntax colors; the changed words of an edited line
    /// carry a stronger fill. A cut line colors only what it shows.
    static func attributed(_ text: String, segments: [ReviewSegment]?, tone: ThemeTone?,
                           highlights: [Grammar.Highlight], inks: SyntaxInks) -> AttributedString {
        var result = AttributedString()
        if let segments, let tone {
            for segment in segments {
                var part = AttributedString(segment.text)
                if segment.changed {
                    part.backgroundColor = tone.ink.opacity(0.26)
                }
                result.append(part)
            }
        } else {
            result = AttributedString(text.isEmpty ? " " : text)
        }
        result.paint(highlights, inks: inks)
        return result
    }

    private func select(_ number: Int, side: ReviewSelection.Side, path: String) {
        store.comments.select(path: path, source: .diff(side: side, revision: revision),
                              line: number, extend: NSEvent.modifierFlags.contains(.shift))
    }
}

/// A changed file's header, in the stream and in the bar above it: the name
/// leads with its directory quiet beside it, then its status, a note (where a
/// rename came from, or the worker's reason), its counts and the way to read it.
struct ReviewFileTitle: View {
    let store: TaskFilesStore
    let file: ReviewFile
    let note: String?
    /// Offers Viewed, which folds the file in the whole comparison.
    var viewable = false
    @Environment(\.palette) private var palette

    var body: some View {
        // One line when it fits, dropping the directory before anything
        // else; otherwise the counts and actions go under the name. Nothing
        // in the row clips.
        ViewThatFits(in: .horizontal) {
            line(directory: true)
            line(directory: false)
            VStack(alignment: .leading, spacing: Spacing.xs) {
                ViewThatFits(in: .horizontal) {
                    title(directory: true)
                    title(directory: false)
                }
                HStack(spacing: Spacing.sm) { actions }
            }
            .frame(maxWidth: .infinity, alignment: .leading)
        }
        .padding(.horizontal, Spacing.md)
        .help(file.path)
    }

    private func line(directory: Bool) -> some View {
        HStack(spacing: Spacing.sm) {
            title(directory: directory)
            Spacer(minLength: 4)
            actions
        }
    }

    private func title(directory showsDirectory: Bool) -> some View {
        let name = (file.path as NSString).lastPathComponent
        let directory = (file.path as NSString).deletingLastPathComponent
        return HStack(spacing: Spacing.sm) {
            HStack(spacing: 0) {
                if showsDirectory, !directory.isEmpty {
                    Text(directory + "/").font(Typography.mono).foregroundStyle(palette.textSecondary)
                }
                Text(name).font(Typography.mono.weight(.semibold))
            }
            .fixedSize()
            if file.status != .modified {
                Text(file.status.rawValue).font(Typography.meta).foregroundStyle(palette.textSecondary).fixedSize()
            }
            if let note, !note.isEmpty {
                Text(note).font(Typography.meta).foregroundStyle(palette.textSecondary)
                    .lineLimit(1).truncationMode(.tail).help(note)
            }
        }
    }

    @ViewBuilder private var actions: some View {
        if !store.diffReview.isReading(file), file.status != .binary {
            Text("+\(file.additions)").foregroundStyle(palette.successTone.ink).font(Typography.mono).fixedSize()
            Text("−\(file.deletions)").foregroundStyle(palette.failureTone.ink).font(Typography.mono).fixedSize()
        }
        if ReviewRendering.of(file)?.hasSource == true {
            let source = store.diffReview.sourceFiles.contains(file.path)
            Button(source ? "Rendered" : "Source") {
                if source { store.diffReview.sourceFiles.remove(file.path) } else { store.diffReview.sourceFiles.insert(file.path) }
            }
            .buttonStyle(.plain).font(Typography.meta).foregroundStyle(palette.textSecondary).fixedSize()
            .help(source ? "Read \(file.path) rendered" : "Show the lines that changed in \(file.path)")
            .accessibilityIdentifier("changes-source")
        }
        Button("Open") { store.selectFile(file.path, mode: .read) }
            .buttonStyle(.plain).font(Typography.meta).foregroundStyle(palette.textSecondary).fixedSize()
            .help("Read \(file.path) in Files").accessibilityLabel("Read \(file.path) in Files")
        if viewable, !store.diffReview.isReading(file) {
            Toggle("Viewed", isOn: Binding(get: { store.diffReview.isViewed(file) }, set: { store.diffReview.setViewed(file, $0) }))
                .toggleStyle(.checkbox).controlSize(.small).font(Typography.meta).fixedSize()
                .help("Fold this file until its lines change")
                .accessibilityIdentifier("changes-viewed")
        }
    }
}

/// A line number: it lights under the pointer, and a click selects the line.
struct ReviewGutter: View {
    let number: Int?
    let selected: Bool
    let tone: ThemeTone?
    let select: () -> Void
    @Environment(\.palette) private var palette
    @State private var hovered = false

    var body: some View {
        Text(number.map(String.init) ?? "")
            .font(Typography.mono).monospacedDigit()
            .foregroundStyle(selected || hovered ? palette.text : tone?.ink.opacity(0.8) ?? palette.textTertiary)
            .frame(width: ReviewStream.gutterWidth, alignment: .trailing)
            .padding(.trailing, Spacing.xs)
            .frame(maxHeight: .infinity)
            .background(hovered && number != nil ? palette.surfaceElement : tone?.fill ?? .clear)
            .contentShape(Rectangle())
            .onHover { hovered = $0 }
            .onTapGesture(perform: select)
            .help(number == nil ? "" : "Select line; ⇧-click extends")
    }
}

/// One outline pulse on the change next/previous just landed on.
/// The accent bar on the change next/previous landed on. Its own view, so
/// stepping redraws the two marks that change, not the stream around them.
private struct ReviewCurrentMark: View {
    let store: TaskFilesStore
    let reader: DiffReviewState.ReviewReader
    let id: String
    @Environment(\.palette) private var palette

    var body: some View {
        if store.diffReview.current(reader) == id {
            ZStack(alignment: .leading) {
                LandingFlash(stepped: store.diffReview.steppedAt, color: palette.accent)
                Rectangle().fill(palette.accent).frame(width: 3).accessibilityLabel("Current change")
            }
            .allowsHitTesting(false)
        }
    }
}

private struct LandingFlash: View {
    let stepped: Date
    let color: Color
    @Environment(\.accessibilityReduceMotion) private var reduceMotion
    @State private var opacity = 0.0

    var body: some View {
        Rectangle().strokeBorder(color, lineWidth: 1.5).opacity(opacity)
            .onAppear {
                guard !reduceMotion, Date.now.timeIntervalSince(stepped) < 0.6 else { return }
                opacity = 1
                withAnimation(.easeOut(duration: 0.9)) { opacity = 0 }
            }
    }
}

/// Under the selected lines: write a comment that stays there, copy the lines,
/// or put a bare reference to them into the conversation's unsent input.
struct ReviewCommentEditor: View {
    @Bindable var store: TaskFilesStore
    let selection: ReviewSelection
    let excerpt: () -> [String]
    let addToConversation: ((String) -> Bool)?
    @Environment(\.palette) private var palette
    @FocusState private var focused: Bool

    var body: some View {
        VStack(alignment: .leading, spacing: Spacing.xs) {
            TextField("Comment on \((selection.path as NSString).lastPathComponent):\(selection.range) · \(selection.source.label)",
                      text: Bindable(store.comments).draft, axis: .vertical)
                .lineLimit(1...5).textFieldStyle(.plain).font(Typography.text)
                .focused($focused)
                .onSubmit { store.comments.add(anchor: excerpt()) }
                .accessibilityIdentifier("changes-comment")
            // One row where it fits; a narrow pane stacks the two pairs.
            ViewThatFits(in: .horizontal) {
                HStack(spacing: Spacing.sm) {
                    commentButton
                    copyButtons
                    Spacer(minLength: 4)
                    cancelButton
                }
                VStack(alignment: .leading, spacing: Spacing.xs) {
                    HStack(spacing: Spacing.sm) {
                        commentButton
                        Spacer(minLength: 4)
                        cancelButton
                    }
                    HStack(spacing: Spacing.sm) { copyButtons }
                }
            }
        }
        .padding(.horizontal, Spacing.sm).padding(.vertical, Spacing.sm)
        .background(palette.surface, in: RoundedRectangle(cornerRadius: CornerRadius.row))
        .overlay { RoundedRectangle(cornerRadius: CornerRadius.row).strokeBorder(palette.accent.opacity(0.6)) }
        .padding(.leading, ReviewStream.gutterWidth + 6).padding(.trailing, Spacing.md).padding(.vertical, Spacing.xs)
        .onAppear { focused = true }
    }

    private var commentButton: some View {
        Button("Comment") { store.comments.add(anchor: excerpt()) }
            .buttonStyle(LoopflowButtonStyle(.accent))
            .disabled(store.comments.draft.allSatisfy(\.isWhitespace))
            .help("Keep this comment under its lines until you send every comment")
    }

    @ViewBuilder private var copyButtons: some View {
        Button("Copy") { ReviewStream.copy(excerpt().joined(separator: "\n")) }
            .buttonStyle(LoopflowButtonStyle())
            .help("Copies the code without + and − or line numbers")
        Button("Reference in conversation") {
            if addToConversation?(selection.reference) == true { store.comments.selection = nil }
        }
        .buttonStyle(LoopflowButtonStyle())
        .disabled(addToConversation == nil)
        .help(addToConversation == nil ? "Open the Task conversation to refer to these lines"
              : "Names these lines in the conversation's unsent input; sending stays yours")
        .accessibilityIdentifier("changes-ask")
    }

    /// Esc is bound once, on the row the pane has room for.
    private var cancelButton: some View {
        Button("Cancel") { store.comments.selection = nil }
            .buttonStyle(.plain).foregroundStyle(palette.textSecondary)
            .keyboardShortcut(.cancelAction)
    }
}

/// A comment waiting under its lines. One whose lines changed says so and
/// keeps the place it had.
struct ReviewCommentRow: View {
    @Bindable var store: TaskFilesStore
    let comment: ReviewComment
    @Environment(\.palette) private var palette

    var body: some View {
        HStack(alignment: .firstTextBaseline, spacing: Spacing.sm) {
            Image(systemName: "text.bubble").foregroundStyle(palette.accent)
            VStack(alignment: .leading, spacing: Spacing.xxs) {
                Text(comment.text).font(Typography.text).foregroundStyle(palette.text)
                    .fixedSize(horizontal: false, vertical: true).textSelection(.enabled)
                Text(place).font(Typography.meta)
                    .foregroundStyle(comment.detached ? palette.warning : palette.textSecondary)
            }
            Spacer(minLength: 4)
            Button { store.comments.items.removeAll { $0.id == comment.id } } label: {
                Image(systemName: "xmark").font(Typography.meta).fontWeight(.semibold).frame(width: 18, height: 18)
                    .contentShape(Rectangle())
            }
            .buttonStyle(.plain).help("Delete this comment").accessibilityLabel("Delete comment")
        }
        .padding(.horizontal, Spacing.sm).padding(.vertical, Spacing.sm)
        .background(palette.accent.opacity(0.08), in: RoundedRectangle(cornerRadius: CornerRadius.row))
        .overlay { RoundedRectangle(cornerRadius: CornerRadius.row).strokeBorder(palette.accent.opacity(0.3)) }
        .padding(.leading, ReviewStream.gutterWidth + 6).padding(.trailing, Spacing.md).padding(.vertical, Spacing.xs)
        .accessibilityElement(children: .contain)
        .accessibilityIdentifier("changes-comment-row")
    }

    private var place: String {
        guard let selection = comment.selection else { return comment.path }
        let lines = "\((selection.path as NSString).lastPathComponent):\(selection.range) · \(selection.source.label)"
        return comment.detached ? "\(lines) · these lines have changed since; still sent" : lines
    }
}

/// A row under the toolbar while comments wait: how many, and the one action
/// that puts them all into the conversation's unsent input as one message.
/// Sending stays yours.
struct ReviewCommentsStrip: View {
    @Bindable var store: TaskFilesStore
    let isFocused: Bool
    let addToConversation: ((String) -> Bool)?
    @Environment(\.palette) private var palette

    var body: some View {
        if !store.comments.items.isEmpty {
            let count = store.comments.items.count
            HStack(spacing: Spacing.sm) {
                Text(count == 1 ? "1 comment" : "\(count) comments").foregroundStyle(palette.accent)
                    .lineLimit(1).accessibilityIdentifier("changes-comment-count")
                Spacer(minLength: 4)
                Button(action: send) {
                    HStack(spacing: Spacing.xs) {
                        Text("Send to conversation")
                        Text("⌘↩").opacity(0.7)
                    }
                    .lineLimit(1)
                    .foregroundStyle(palette.background)
                    .padding(.horizontal, Spacing.sm).frame(height: 20)
                    .background(addToConversation == nil ? palette.textTertiary : palette.accent, in: RoundedRectangle(cornerRadius: CornerRadius.control))
                    .contentShape(Rectangle())
                }
                .buttonStyle(.plain)
                .keyboardShortcut(isFocused ? KeyboardShortcut(.return, modifiers: .command) : nil)
                .disabled(addToConversation == nil)
                .help(addToConversation == nil ? "Open the Task conversation to send these comments"
                      : "Puts every comment into the conversation's unsent input as one message; sending stays yours")
                .accessibilityIdentifier("changes-send")
            }
            .font(Typography.meta)
            .padding(.horizontal, Spacing.md).padding(.vertical, Spacing.xs)
            .background(palette.accent.opacity(0.08))
            Divider()
        }
    }

    /// Comments are spent once they are in the conversation; a failed send keeps them.
    private func send() {
        if let addToConversation { store.comments.insert(into: addToConversation) }
    }
}

struct ReviewNotice: View {
    let text: String
    @Environment(\.palette) private var palette

    var body: some View {
        Text(text).font(Typography.text).foregroundStyle(palette.textSecondary)
            .fixedSize(horizontal: false, vertical: true)
            .reviewNoticeInset()
    }
}

private extension View {
    /// Where a sentence sits among the stream's rows.
    func reviewNoticeInset() -> some View { padding(.horizontal, Spacing.md).padding(.vertical, Spacing.xs) }
}

/// Under a file's lines: why they stop short, or that they were not read,
/// with the Machine's message and the way to read them again.
struct ReviewFileNote: View {
    let note: DiffReviewState.FileNote
    let retry: () -> Void

    var body: some View {
        switch note {
        case .cut:
            ReviewNotice(text: "Cut at 1 MB. The rest of this file is not shown.")
        case .failed(let reason):
            FailureLine("File not read.", machine: reason) {
                Button("Retry", action: retry)
            }
            .reviewNoticeInset()
        }
    }
}

/// Previous/next change around the position, with `[`/`]` while the pane has
/// focus. Diff steps across the review stream. Files steps within the open file.
struct ReviewStepper: View {
    @Bindable var store: TaskFilesStore
    let reader: DiffReviewState.ReviewReader
    let isFocused: Bool
    @Environment(\.palette) private var palette

    var body: some View {
        HStack(spacing: 0) {
            arrow("chevron.up", "Previous change", key: "[", offset: -1)
            Divider().frame(height: 14)
            Text(store.diffReview.position(reader)).font(Typography.meta).foregroundStyle(palette.text)
                .monospacedDigit().fixedSize().padding(.horizontal, Spacing.sm).frame(minWidth: 54)
                .accessibilityIdentifier("review-position")
            Divider().frame(height: 14)
            arrow("chevron.down", "Next change", key: "]", offset: 1)
        }
        .background(palette.surface, in: RoundedRectangle(cornerRadius: CornerRadius.row))
        .overlay { RoundedRectangle(cornerRadius: CornerRadius.row).strokeBorder(palette.border) }
        .background {
            HStack {
                Button("Next change") { go(1) }.keyboardShortcut("]", modifiers: [])
                Button("Previous change") { go(-1) }.keyboardShortcut("[", modifiers: [])
            }
            .disabled(!isFocused).hidden().accessibilityHidden(true).frame(width: 0, height: 0)
        }
    }

    private func arrow(_ symbol: String, _ label: String, key: String, offset: Int) -> some View {
        Button { go(offset) } label: {
            Image(systemName: symbol).font(Typography.meta).fontWeight(.semibold)
                .frame(width: 26, height: 20).contentShape(Rectangle())
        }
        .buttonStyle(.plain).help("\(label) (\(key))").accessibilityLabel(label)
    }

    private func go(_ offset: Int) {
        guard let stop = store.diffReview.step(offset, in: reader) else { return }
        withAnimation(.easeOut(duration: 0.15)) { store.diffReview[place: reader].anchor = stop.id }
    }
}

extension DiffReviewState {
    /// The reader's scroll anchor, for `scrollPosition(id:)`.
    func anchor(_ reader: ReviewReader) -> Binding<String?> {
        Binding(get: { self[place: reader].anchor }, set: { self.scrolled(to: $0, in: reader) })
    }
}

/// New code, unified or side by side. Side by side needs a wide pane; a
/// narrow one says so and reads unified.
struct ReviewLayoutPicker: View {
    @Bindable var store: TaskFilesStore
    let width: CGFloat
    @Environment(\.palette) private var palette

    var body: some View {
        HStack(spacing: 0) {
            segment(.new, help: "The file as it is now, removed lines folded")
            Divider().frame(height: 14)
            segment(.unified, help: "Removed and added lines together")
            Divider().frame(height: 14)
            segment(.split,
                    help: narrow ? "Side by side needs a pane at least \(Int(DiffReviewState.splitWidth)) pt wide" : "Side by side")
                .disabled(narrow)
        }
        .background(palette.surface, in: RoundedRectangle(cornerRadius: CornerRadius.row))
        .overlay { RoundedRectangle(cornerRadius: CornerRadius.row).strokeBorder(palette.border) }
        .clipShape(RoundedRectangle(cornerRadius: CornerRadius.row))
    }

    private var narrow: Bool { width < DiffReviewState.splitWidth }

    private func segment(_ layout: DiffReviewState.ReviewLayout, help: String) -> some View {
        // What the stream is actually reading, so a narrow pane shows unified as on.
        let on = layout == store.diffReview.layout(width: width)
        return Button { store.diffReview.reviewLayout = layout } label: {
            Text(layout.rawValue).font(Typography.text).lineLimit(1).fixedSize()
                .foregroundStyle(on ? palette.background : palette.text)
                .padding(.horizontal, Spacing.sm).frame(height: 20)
                .background(on ? palette.text : .clear)
                .contentShape(Rectangle())
        }
        .buttonStyle(.plain).help(help).accessibilityLabel(layout.rawValue)
        .accessibilityAddTraits(on ? .isSelected : [])
    }
}
#endif
