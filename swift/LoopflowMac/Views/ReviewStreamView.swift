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
    let reader: TaskFilesStore.ReviewReader
    let files: [ReviewFile]
    /// Side by side; callers fall back to unified in a narrow pane.
    let split: Bool
    /// Words to read above a file, by its path.
    var headings: [String: String] = [:]
    /// False when the caller names the first file itself, above the stream.
    var headsFirstFile = true
    let addToConversation: ((String) -> Bool)?
    @Environment(\.palette) private var palette

    var body: some View {
        if let error = store.gapError { ReviewNotice(text: error) }
        if reader == .changes {
            // A comment whose file left the comparison still waits to be sent.
            ForEach(store.comments.filter { store.review?.file($0.selection.path) == nil }) { comment in
                ReviewCommentRow(store: store, comment: comment)
            }
        }
        ForEach(files) { unopened in
            let file = store.opened(unopened)
            // Reading state and notes belong to the whole comparison's per-file reads.
            let note = reader == .changes ? store.fileNotes[file.path] : nil
            if let heading = headings[file.path] {
                Text(heading).font(Typography.textStrong).foregroundStyle(palette.text)
                    .fixedSize(horizontal: false, vertical: true)
                    .padding(.horizontal, Spacing.md).padding(.top, Spacing.xl)
                    .accessibilityAddTraits(.isHeader)
            }
            if headsFirstFile || file.path != files.first?.path {
                fileHeader(file, headed: headings[file.path] != nil).id("file:\(file.path)")
            }
            ForEach(store.comments.filter { $0.detached && $0.selection.path == file.path }) { comment in
                ReviewCommentRow(store: store, comment: comment)
            }
            if reader == .changes, store.isViewed(file) {
                // Viewed: folded to its header until you clear it or its lines change.
            } else if reader == .changes, store.isReading(file) {
                ReviewNotice(text: "Reading…").id(file.topID)
                    .onAppear { store.setNear(file.path, true) }
                    .onDisappear { store.setNear(file.path, false) }
            } else {
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
            if let note {
                HStack(spacing: 0) {
                    ReviewNotice(text: note.text)
                    if case .failed = note {
                        Button("Retry") { Task { await store.retryFile(file.path) } }
                            .buttonStyle(WorkPlainButtonStyle()).font(Typography.meta)
                    }
                }
            }
        }
    }

    static let gutterWidth: CGFloat = 36

    /// Where new code dropped lines: how many, and the way to read them.
    private func removalMark(_ count: Int, change: String) -> some View {
        Button { store.openedRemovals.insert(change) } label: {
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
            gapRow(header, gap: gap, path: file.path).id(block.id(in: file.path))
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
        if store.reviewLayout == .new, !store.openedRemovals.contains(change.id) {
            if !change.removed.isEmpty { rows.append(.removals(change.removed.count)) }
        } else {
            rows += change.pairs.compactMap { pair in pair.removed.map { .line($0, pair.removedSegments) } }
        }
        rows += change.pairs.compactMap { pair in pair.added.map { .line($0, pair.addedSegments) } }
        return rows
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
    @ViewBuilder private func gapRow(_ header: String, gap: ReviewGap?, path: String) -> some View {
        let label = gap?.label ?? ""
        let row = HStack(spacing: Spacing.sm) {
            if !label.isEmpty {
                Text(label).font(Typography.meta).foregroundStyle(palette.textTertiary)
                    .lineLimit(1).truncationMode(.tail)
            }
            Rectangle().fill(palette.border).frame(height: 1)
        }
        .padding(.leading, Self.gutterWidth + 6).padding(.trailing, Spacing.md).padding(.vertical, Spacing.xs)
        if let gap, !gap.new.isEmpty, store.canOpenGaps {
            Button {
                Task { await store.openGap(header, gap: gap, path: path, reader: reader) }
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
        let expanded = store.expandedMoves.contains(move.id)
        return VStack(alignment: .leading, spacing: 0) {
            Button {
                if move.end == .source {
                    store[place: reader] = .init(anchor: move.counterpart, current: move.counterpart)
                } else if expanded {
                    store.expandedMoves.remove(move.id)
                } else {
                    store.expandedMoves.insert(move.id)
                }
                store.noteInteraction()
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
        if let selection = store.reviewSelection, selection.path == file.path,
           selection.last == (selection.side == .old ? old : new) {
            ReviewCommentEditor(store: store, selection: selection, file: file, addToConversation: addToConversation)
        }
        ForEach(store.comments.filter { comment in
            !comment.detached && comment.selection.path == file.path
                && comment.selection.last == (comment.selection.side == .old ? old : new)
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
        store.reviewSelection.map {
            $0.contains(line, path: path) && (line.kind == .context || $0.side == side)
        } ?? false
    }

    private func cell(_ line: ReviewLine, segments: [ReviewSegment]?, side: ReviewSelection.Side, file: ReviewFile) -> some View {
        let path = file.path
        let tone = tone(of: line, in: file)
        let selected = isSelected(line, side: side, path: path)
        // A line past the readable length shows its start until asked for the rest.
        let cut = line.cut.flatMap { store.openedLongLines.contains(line.id(in: path)) ? nil : $0 }
        let text = cut?.shown ?? line.text
        return VStack(alignment: .leading, spacing: Spacing.xxs) {
            HStack(alignment: .top, spacing: Spacing.xs) {
                // A marker as well as color: + and − read without hue.
                // Selecting and copying takes the code only.
                Text(tone == nil ? " " : line.kind == .added ? "+" : "−")
                    .foregroundStyle(tone?.ink ?? palette.textSecondary)
                    .textSelection(.disabled)
                Text(Self.attributed(text, segments: cut == nil ? segments : nil, tone: tone, path: path, palette: palette))
                    .lineLimit(store.wrapsLines ? nil : 1)
                    .frame(maxWidth: .infinity, alignment: .leading)
                    .fixedSize(horizontal: false, vertical: true)
                    .textSelection(.enabled)
            }
            .font(Typography.mono)
            .accessibilityElement(children: .combine)
            .accessibilityLabel("\(line.kind == .added ? "Added" : line.kind == .removed ? "Removed" : "Line") \(side == .old ? line.old ?? 0 : line.new ?? 0): \(text)")
            if let cut {
                Button("\(cut.more.formatted()) more characters · Show") { store.openedLongLines.insert(line.id(in: path)) }
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
        if let selection = store.reviewSelection, selection.path == file.path, selection.side == side,
           selection.first != selection.last, isSelected(line, side: side, path: file.path) {
            Button("Copy \(selection.last - selection.first + 1) selected lines") { Self.copy(selection.text(in: file)) }
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
    /// carry a stronger fill.
    static func attributed(_ text: String, segments: [ReviewSegment]?, tone: ThemeTone?,
                           path: String, palette: LoopflowPalette) -> AttributedString {
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
        result.colorSyntax(of: text, path: path, palette: palette)
        return result
    }

    private func select(_ number: Int, side: ReviewSelection.Side, path: String) {
        let extend = NSEvent.modifierFlags.contains(.shift)
        if extend, let current = store.reviewSelection, current.path == path, current.side == side {
            store.reviewSelection = current.extended(to: number)
        } else {
            store.reviewSelection = ReviewSelection(path: path, side: side, first: number, last: number)
        }
        store.noteInteraction()
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
        if !store.isReading(file) {
            Text("+\(file.additions)").foregroundStyle(palette.successTone.ink).font(Typography.mono).fixedSize()
            Text("−\(file.deletions)").foregroundStyle(palette.failureTone.ink).font(Typography.mono).fixedSize()
        }
        Button("Open") { store.selectFile(file.path, mode: .read) }
            .buttonStyle(.plain).font(Typography.meta).foregroundStyle(palette.textSecondary).fixedSize()
            .help("Read \(file.path) in Files").accessibilityLabel("Read \(file.path) in Files")
        if viewable, !store.isReading(file) {
            Toggle("Viewed", isOn: Binding(get: { store.isViewed(file) }, set: { store.setViewed(file, $0) }))
                .toggleStyle(.checkbox).controlSize(.small).font(Typography.meta).fixedSize()
                .help("Fold this file until its lines change")
                .accessibilityIdentifier("changes-viewed")
        }
    }
}

/// A line number: it lights under the pointer, and a click selects the line.
private struct ReviewGutter: View {
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
    let reader: TaskFilesStore.ReviewReader
    let id: String
    @Environment(\.palette) private var palette

    var body: some View {
        if store.current(reader) == id {
            ZStack(alignment: .leading) {
                LandingFlash(stepped: store.steppedAt, color: palette.accent)
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
private struct ReviewCommentEditor: View {
    @Bindable var store: TaskFilesStore
    let selection: ReviewSelection
    /// The file as the stream shows it, opened lines included.
    let file: ReviewFile
    let addToConversation: ((String) -> Bool)?
    @Environment(\.palette) private var palette
    @FocusState private var focused: Bool

    var body: some View {
        VStack(alignment: .leading, spacing: Spacing.xs) {
            TextField("Comment on \((selection.path as NSString).lastPathComponent):\(selection.range) · \(selection.side.rawValue)",
                      text: $store.commentDraft, axis: .vertical)
                .lineLimit(1...5).textFieldStyle(.plain).font(Typography.text)
                .focused($focused)
                .onSubmit { store.addComment(in: file) }
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
        Button("Comment") { store.addComment(in: file) }
            .buttonStyle(LoopflowButtonStyle(.accent))
            .disabled(store.commentDraft.allSatisfy(\.isWhitespace))
            .help("Keep this comment under its lines until you send every comment")
    }

    @ViewBuilder private var copyButtons: some View {
        Button("Copy") { ReviewStream.copy(selection.text(in: file)) }
            .buttonStyle(LoopflowButtonStyle())
            .help("Copies the code without + and − or line numbers")
        Button("Reference in conversation") {
            if addToConversation?(selection.reference(revision: store.revision)) == true { store.reviewSelection = nil }
        }
        .buttonStyle(LoopflowButtonStyle())
        .disabled(addToConversation == nil)
        .help(addToConversation == nil ? "Open the Task conversation to refer to these lines"
              : "Names these lines in the conversation's unsent input; sending stays yours")
        .accessibilityIdentifier("changes-ask")
    }

    /// Esc is bound once, on the row the pane has room for.
    private var cancelButton: some View {
        Button("Cancel") { store.reviewSelection = nil }
            .buttonStyle(.plain).foregroundStyle(palette.textSecondary)
            .keyboardShortcut(.cancelAction)
    }
}

/// A comment waiting under its lines. One whose lines changed says so and
/// keeps the place it had.
private struct ReviewCommentRow: View {
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
            Button { store.comments.removeAll { $0.id == comment.id } } label: {
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
        let selection = comment.selection
        let lines = "\((selection.path as NSString).lastPathComponent):\(selection.range) · \(selection.side.rawValue)"
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
        if !store.comments.isEmpty {
            let count = store.comments.count
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
        guard let addToConversation, !store.comments.isEmpty else { return }
        let message = ReviewComment.message(store.comments, revision: store.revision)
        if addToConversation(message) { store.comments = [] }
    }
}

struct ReviewNotice: View {
    let text: String
    @Environment(\.palette) private var palette

    var body: some View {
        Text(text).font(Typography.text).foregroundStyle(palette.textSecondary)
            .fixedSize(horizontal: false, vertical: true)
            .padding(.horizontal, Spacing.md).padding(.vertical, Spacing.xs)
    }
}

/// Previous/next change around the position, with `[`/`]` while the pane has
/// focus. Changes crosses files in its stream; Files' Diff opens the
/// neighbouring changed file past either end.
struct ReviewStepper: View {
    @Bindable var store: TaskFilesStore
    let reader: TaskFilesStore.ReviewReader
    let isFocused: Bool
    @Environment(\.palette) private var palette

    var body: some View {
        HStack(spacing: 0) {
            arrow("chevron.up", "Previous change", key: "[", offset: -1)
            Divider().frame(height: 14)
            Text(store.position(reader)).font(Typography.meta).foregroundStyle(palette.text)
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
        guard let stop = store.step(offset, in: reader) else { return }
        withAnimation(.easeOut(duration: 0.15)) { store[place: reader].anchor = stop.id }
    }
}

extension TaskFilesStore {
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
                    help: narrow ? "Side by side needs a pane at least \(Int(TaskFilesStore.splitWidth)) pt wide" : "Side by side")
                .disabled(narrow)
        }
        .background(palette.surface, in: RoundedRectangle(cornerRadius: CornerRadius.row))
        .overlay { RoundedRectangle(cornerRadius: CornerRadius.row).strokeBorder(palette.border) }
        .clipShape(RoundedRectangle(cornerRadius: CornerRadius.row))
    }

    private var narrow: Bool { width < TaskFilesStore.splitWidth }

    private func segment(_ layout: TaskFilesStore.ReviewLayout, help: String) -> some View {
        // What the stream is actually reading, so a narrow pane shows unified as on.
        let on = layout == store.layout(width: width)
        return Button { store.reviewLayout = layout } label: {
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
