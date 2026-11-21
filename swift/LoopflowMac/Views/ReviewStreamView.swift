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
            if let heading = headings[file.path] {
                Text(heading).font(Typography.strong(13)).foregroundStyle(palette.text)
                    .fixedSize(horizontal: false, vertical: true)
                    .padding(.horizontal, 12).padding(.top, 22)
                    .accessibilityAddTraits(.isHeader)
            }
            if headsFirstFile || file.path != files.first?.path {
                fileHeader(file, headed: headings[file.path] != nil).id("file:\(file.path)")
            }
            ForEach(store.comments.filter { $0.detached && $0.selection.path == file.path }) { comment in
                ReviewCommentRow(store: store, comment: comment)
            }
            if reader == .changes, store.isReading(file) {
                ReviewNotice(text: "Reading…").id(file.topID)
                    .onAppear { store.setNear(file.path, true) }
                    .onDisappear { store.setNear(file.path, false) }
            } else if store.collapses(file) {
                longLines(file).id(file.topID)
            } else {
                if file.status == .binary {
                    ReviewNotice(text: "Binary file · no text to compare").id(file.topID)
                } else if file.blocks.isEmpty, reader != .changes || store.fileNotes[file.path] == nil {
                    ReviewNotice(text: file.status == .renamed ? "Renamed without changes" : "No textual changes")
                        .id(file.topID)
                }
                let gaps = file.gaps
                ForEach(Array(file.blocks.enumerated()), id: \.offset) { index, block in
                    if file.status == .added, case .change(let change) = block {
                        // A new file is its contents: one lazy row per line, however long it is.
                        ForEach(Array(change.added.enumerated()), id: \.offset) { offset, line in
                            lineRow(line, segments: nil, file: file)
                                .overlay { if offset == 0 { currentMark(change.id) } }
                                .id(offset == 0 ? change.id : "\(change.id)+\(offset)")
                        }
                    } else {
                        blockView(block, gap: gaps[index], file: file).id(block.id(in: file.path))
                    }
                }
            }
            if reader == .changes, let note = store.fileNotes[file.path] {
                ReviewNotice(text: note)
            }
        }
    }

    static let gutterWidth: CGFloat = 36

    /// In place of lines nobody reads: what the file holds, and the ways to see it.
    private func longLines(_ file: ReviewFile) -> some View {
        let lines = file.additions + file.deletions
        let size = ByteCountFormatter.string(fromByteCount: Int64(file.bytes), countStyle: .file)
        return HStack(spacing: 10) {
            Text("Lines too long to read · \(lines == 1 ? "1 changed line" : "\(lines) changed lines") · \(size)")
                .foregroundStyle(palette.textSecondary).fixedSize(horizontal: false, vertical: true)
            Spacer(minLength: 4)
            Button("Show") { store.openedLongFiles.insert(file.path) }
            Button("Open in Files") { store.selectFile(file.path, mode: .read) }
        }
        .buttonStyle(WorkPlainButtonStyle()).font(Typography.caption(11))
        .padding(.horizontal, 12).padding(.vertical, 8)
        .overlay { if let first = file.blocks.first(where: \.isStop) { currentMark(first.id(in: file.path)) } }
        .accessibilityElement(children: .contain)
        .accessibilityIdentifier("changes-long-lines")
    }

    private func fileHeader(_ file: ReviewFile, headed: Bool) -> some View {
        ReviewFileTitle(store: store, file: file, note: file.oldPath.map { "from \($0)" })
            .padding(.vertical, 7)
            .background(palette.surfaceMuted)
            .overlay(alignment: .top) { Divider() }
            .padding(.top, headed ? 6 : 16)
            .contextMenu {
                Button("Open in Files") { store.selectFile(file.path, mode: .diff) }
                Button("Read file") { store.selectFile(file.path, mode: .read) }
            }
    }

    @ViewBuilder
    private func blockView(_ block: ReviewBlock, gap: ReviewGap?, file: ReviewFile) -> some View {
        switch block {
        case .hunk(let header):
            gapRow(header, gap: gap, path: file.path)
        case .context(let line):
            pairRow(ReviewPair(removed: line, added: line, removedSegments: nil, addedSegments: nil),
                    file: file, context: true)
        case .change(let change):
            VStack(spacing: 0) {
                if split, file.status != .added {
                    ForEach(Array(change.pairs.enumerated()), id: \.offset) { _, pair in
                        pairRow(pair, file: file, context: false)
                    }
                } else {
                    // Unified: every removed line, then every added line.
                    let pairs = Array(change.pairs.enumerated())
                    ForEach(pairs, id: \.offset) { _, pair in
                        if let line = pair.removed { lineRow(line, segments: pair.removedSegments, file: file) }
                    }
                    ForEach(pairs, id: \.offset) { _, pair in
                        if let line = pair.added { lineRow(line, segments: pair.addedSegments, file: file) }
                    }
                }
            }
            .overlay { currentMark(change.id) }
        case .moved(let move):
            movedView(move, file: file)
                .overlay { currentMark(move.id) }
        }
    }

    /// The lines between two hunks: how many, and the declaration git names.
    /// A click reads them from the file and shows them in place.
    @ViewBuilder private func gapRow(_ header: String, gap: ReviewGap?, path: String) -> some View {
        let label = gap?.label ?? ""
        let row = HStack(spacing: 8) {
            if !label.isEmpty {
                Text(label).font(Typography.caption(11)).foregroundStyle(palette.textTertiary)
                    .lineLimit(1).truncationMode(.tail)
            }
            Rectangle().fill(palette.border).frame(height: 1)
        }
        .padding(.leading, Self.gutterWidth + 6).padding(.trailing, 12).padding(.vertical, 5)
        if let gap, !gap.new.isEmpty, store.canOpenGaps {
            Button {
                Task { await store.openGap(header, gap: gap, path: path, reader: reader) }
            } label: {
                row.overlay(alignment: .leading) {
                    Image(systemName: "arrow.up.and.down").font(.system(size: 9, weight: .semibold))
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

    @ViewBuilder private func currentMark(_ id: String) -> some View {
        if store[place: reader].current == id {
            ZStack(alignment: .leading) {
                LandingFlash(stepped: store.steppedAt, color: palette.accent)
                Rectangle().fill(palette.accent).frame(width: 3).accessibilityLabel("Current change")
            }
            .allowsHitTesting(false)
        }
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
                HStack(spacing: 6) {
                    Image(systemName: move.end == .source ? "arrow.turn.right.down" : "arrow.left.arrow.right")
                        .frame(width: 14)
                    Text(Self.movedLabel(move))
                    Spacer(minLength: 0)
                    if move.end == .destination { Image(systemName: expanded ? "chevron.down" : "chevron.right") }
                }
                .font(Typography.caption(12))
                .foregroundStyle(palette.text)
                .padding(.leading, Self.gutterWidth + 6).padding(.trailing, 12).padding(.vertical, 5)
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
                gutter(side == .old ? line.old : line.new, line: line, side: side, path: file.path)
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
                        gutter(pair.removed?.old, line: pair.removed, side: .old, path: file.path)
                        if let line = pair.removed { cell(line, segments: pair.removedSegments, side: .old, file: file) }
                        else { empty }
                    }
                    Divider()
                    HStack(spacing: 0) {
                        gutter(pair.added?.new, line: pair.added, side: .new, path: file.path)
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

    private func gutter(_ number: Int?, line: ReviewLine?, side: ReviewSelection.Side, path: String) -> some View {
        ReviewGutter(number: number, selected: line.map { isSelected($0, side: side, path: path) } ?? false) {
            if let number { select(number, side: side, path: path) }
        }
    }

    private func isSelected(_ line: ReviewLine, side: ReviewSelection.Side, path: String) -> Bool {
        store.reviewSelection.map {
            $0.contains(line, path: path) && (line.kind == .context || $0.side == side)
        } ?? false
    }

    private func cell(_ line: ReviewLine, segments: [ReviewSegment]?, side: ReviewSelection.Side, file: ReviewFile) -> some View {
        let path = file.path
        // A new file reads as its contents, without the marks of a patch.
        let tone: ThemeTone? = switch line.kind {
        case .added: file.status == .added ? nil : palette.successTone
        case .removed: palette.failureTone
        case .context: nil
        }
        let selected = isSelected(line, side: side, path: path)
        return HStack(alignment: .top, spacing: 4) {
            // A marker as well as color: + and − read without hue.
            Text(tone == nil ? " " : line.kind == .added ? "+" : "−")
                .foregroundStyle(tone?.ink ?? palette.textSecondary)
            Text(Self.attributed(line.text, segments: segments, tone: tone, path: path, palette: palette))
                .frame(maxWidth: .infinity, alignment: .leading)
                .fixedSize(horizontal: false, vertical: true)
        }
        .font(Typography.code(12))
        .padding(.vertical, 1).padding(.trailing, 8)
        .frame(maxWidth: .infinity, alignment: .leading)
        .background(selected ? palette.accent.opacity(0.16) : tone?.fill.opacity(0.4) ?? .clear)
        .textSelection(.enabled)
        .accessibilityElement(children: .combine)
        .accessibilityLabel("\(line.kind == .added ? "Added" : line.kind == .removed ? "Removed" : "Line") \(side == .old ? line.old ?? 0 : line.new ?? 0): \(line.text)")
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
    @Environment(\.palette) private var palette

    var body: some View {
        let name = (file.path as NSString).lastPathComponent
        let directory = (file.path as NSString).deletingLastPathComponent
        HStack(spacing: 8) {
            HStack(spacing: 0) {
                if !directory.isEmpty {
                    Text(directory + "/").font(Typography.code(11)).foregroundStyle(palette.textSecondary)
                        .lineLimit(1).truncationMode(.head)
                }
                Text(name).font(Typography.code(12).weight(.semibold)).lineLimit(1).layoutPriority(1)
            }
            if file.status != .modified {
                Text(file.status.rawValue).font(Typography.caption(11)).foregroundStyle(palette.textSecondary)
            }
            if let note, !note.isEmpty {
                Text(note).font(Typography.caption(11)).foregroundStyle(palette.textSecondary)
                    .lineLimit(1).truncationMode(.tail).help(note)
            }
            Spacer(minLength: 4)
            if !store.isReading(file) {
                Text("+\(file.additions)").foregroundStyle(palette.successTone.ink).font(Typography.code(11)).fixedSize()
                Text("−\(file.deletions)").foregroundStyle(palette.failureTone.ink).font(Typography.code(11)).fixedSize()
            }
            Button("Open") { store.selectFile(file.path, mode: .read) }
                .buttonStyle(.plain).font(Typography.caption(11)).foregroundStyle(palette.textSecondary).fixedSize()
                .help("Read \(file.path) in Files").accessibilityLabel("Read \(file.path) in Files")
        }
        .padding(.horizontal, 12)
        .help(file.path)
    }
}

/// A line number: it lights under the pointer, and a click selects the line.
private struct ReviewGutter: View {
    let number: Int?
    let selected: Bool
    let select: () -> Void
    @Environment(\.palette) private var palette
    @State private var hovered = false

    var body: some View {
        Text(number.map(String.init) ?? "")
            .font(Typography.code(11)).monospacedDigit()
            .foregroundStyle(selected || hovered ? palette.text : palette.textTertiary)
            .frame(width: ReviewStream.gutterWidth, alignment: .trailing)
            .padding(.trailing, 6)
            .frame(maxHeight: .infinity)
            .background(hovered && number != nil ? palette.surfaceElement : .clear)
            .contentShape(Rectangle())
            .onHover { hovered = $0 }
            .onTapGesture(perform: select)
            .help(number == nil ? "" : "Select line; ⇧-click extends")
    }
}

/// One outline pulse on the change next/previous just landed on.
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
        VStack(alignment: .leading, spacing: 6) {
            TextField("Comment on \((selection.path as NSString).lastPathComponent):\(selection.range) · \(selection.side.rawValue)",
                      text: $store.commentDraft, axis: .vertical)
                .lineLimit(1...5).textFieldStyle(.plain).font(Typography.body(12))
                .focused($focused)
                .onSubmit { store.addComment(in: file) }
                .accessibilityIdentifier("changes-comment")
            // One row where it fits; a narrow pane stacks the two pairs.
            ViewThatFits(in: .horizontal) {
                HStack(spacing: 8) {
                    commentButton
                    copyButtons
                    Spacer(minLength: 4)
                    cancelButton
                }
                VStack(alignment: .leading, spacing: 6) {
                    HStack(spacing: 8) {
                        commentButton
                        Spacer(minLength: 4)
                        cancelButton
                    }
                    HStack(spacing: 8) { copyButtons }
                }
            }
            .controlSize(.small)
            .font(Typography.caption(11))
        }
        .padding(.horizontal, 10).padding(.vertical, 8)
        .background(palette.surface, in: RoundedRectangle(cornerRadius: 8))
        .overlay { RoundedRectangle(cornerRadius: 8).strokeBorder(palette.accent.opacity(0.6)) }
        .padding(.leading, ReviewStream.gutterWidth + 6).padding(.trailing, 12).padding(.vertical, 5)
        .onAppear { focused = true }
    }

    private var commentButton: some View {
        Button("Comment") { store.addComment(in: file) }
            .buttonStyle(.borderedProminent)
            .disabled(store.commentDraft.allSatisfy(\.isWhitespace))
            .help("Keep this comment under its lines until you send every comment")
    }

    @ViewBuilder private var copyButtons: some View {
        Button("Copy") {
            NSPasteboard.general.clearContents()
            NSPasteboard.general.setString(selection.text(in: file), forType: .string)
        }
        .buttonStyle(.bordered)
        Button("Reference in conversation") {
            if addToConversation?(selection.reference(revision: store.revision)) == true { store.reviewSelection = nil }
        }
        .buttonStyle(.bordered)
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
        HStack(alignment: .firstTextBaseline, spacing: 8) {
            Image(systemName: "text.bubble").foregroundStyle(palette.accent)
            VStack(alignment: .leading, spacing: 2) {
                Text(comment.text).font(Typography.body(12)).foregroundStyle(palette.text)
                    .fixedSize(horizontal: false, vertical: true).textSelection(.enabled)
                Text(place).font(Typography.caption(11))
                    .foregroundStyle(comment.detached ? palette.warning : palette.textSecondary)
            }
            Spacer(minLength: 4)
            Button { store.comments.removeAll { $0.id == comment.id } } label: {
                Image(systemName: "xmark").font(.system(size: 9, weight: .semibold)).frame(width: 18, height: 18)
                    .contentShape(Rectangle())
            }
            .buttonStyle(.plain).help("Delete this comment").accessibilityLabel("Delete comment")
        }
        .padding(.horizontal, 10).padding(.vertical, 7)
        .background(palette.accent.opacity(0.08), in: RoundedRectangle(cornerRadius: 8))
        .overlay { RoundedRectangle(cornerRadius: 8).strokeBorder(palette.accent.opacity(0.3)) }
        .padding(.leading, ReviewStream.gutterWidth + 6).padding(.trailing, 12).padding(.vertical, 4)
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
            HStack(spacing: 8) {
                Text(count == 1 ? "1 comment" : "\(count) comments").foregroundStyle(palette.accent)
                    .lineLimit(1).accessibilityIdentifier("changes-comment-count")
                Spacer(minLength: 4)
                Button(action: send) {
                    HStack(spacing: 5) {
                        Text("Send to conversation")
                        Text("⌘↩").opacity(0.7)
                    }
                    .lineLimit(1)
                    .foregroundStyle(palette.background)
                    .padding(.horizontal, 9).frame(height: 20)
                    .background(addToConversation == nil ? palette.textTertiary : palette.accent, in: RoundedRectangle(cornerRadius: 6))
                    .contentShape(Rectangle())
                }
                .buttonStyle(.plain)
                .keyboardShortcut(isFocused ? KeyboardShortcut(.return, modifiers: .command) : nil)
                .disabled(addToConversation == nil)
                .help(addToConversation == nil ? "Open the Task conversation to send these comments"
                      : "Puts every comment into the conversation's unsent input as one message; sending stays yours")
                .accessibilityIdentifier("changes-send")
            }
            .font(Typography.caption(11))
            .padding(.horizontal, 12).padding(.vertical, 5)
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
        Text(text).font(.caption).foregroundStyle(palette.textSecondary)
            .padding(.horizontal, 14).padding(.vertical, 6)
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
            HStack(spacing: 0) {
                arrow("chevron.up", "Previous change", key: "[", offset: -1)
                Divider().frame(height: 14)
                Text(store.position(reader)).font(Typography.caption(11)).foregroundStyle(palette.text)
                    .monospacedDigit().lineLimit(1).padding(.horizontal, 8).frame(minWidth: 54)
                    .accessibilityIdentifier("review-position")
                Divider().frame(height: 14)
                arrow("chevron.down", "Next change", key: "]", offset: 1)
            }
            .background(palette.surface, in: RoundedRectangle(cornerRadius: 6))
            .overlay { RoundedRectangle(cornerRadius: 6).strokeBorder(palette.border) }
        }
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
            Image(systemName: symbol).font(.system(size: 10, weight: .semibold))
                .frame(width: 26, height: 20).contentShape(Rectangle())
        }
        .buttonStyle(.plain).help("\(label) (\(key))").accessibilityLabel(label)
    }

    private func go(_ offset: Int) {
        guard let stop = store.step(offset, in: reader) else { return }
        withAnimation(.easeOut(duration: 0.15)) { store[place: reader].anchor = store.row(for: stop, in: reader) }
    }
}

extension TaskFilesStore {
    /// The reader's scroll anchor, for `scrollPosition(id:)`.
    func anchor(_ reader: ReviewReader) -> Binding<String?> {
        Binding(get: { self[place: reader].anchor }, set: { self[place: reader].anchor = $0 })
    }
}

/// Unified or side by side. Side by side needs a wide pane; a narrow one says
/// so and reads unified.
struct ReviewLayoutPicker: View {
    @Bindable var store: TaskFilesStore
    let width: CGFloat
    @Environment(\.palette) private var palette

    var body: some View {
        HStack(spacing: 0) {
            segment(.unified, symbol: "text.justify.left", help: "Unified")
            Divider().frame(height: 14)
            segment(.split, symbol: "rectangle.split.2x1",
                    help: narrow ? "Side by side needs a pane at least \(Int(TaskFilesStore.splitWidth)) pt wide" : "Side by side")
                .disabled(narrow)
        }
        .background(palette.surface, in: RoundedRectangle(cornerRadius: 6))
        .overlay { RoundedRectangle(cornerRadius: 6).strokeBorder(palette.border) }
        .clipShape(RoundedRectangle(cornerRadius: 6))
    }

    private var narrow: Bool { width < TaskFilesStore.splitWidth }

    private func segment(_ layout: TaskFilesStore.ReviewLayout, symbol: String, help: String) -> some View {
        // What the stream is actually reading, so a narrow pane shows unified as on.
        let on = (layout == .split) == store.readsSplit(width: width)
        return Button { store.reviewLayout = layout } label: {
            Image(systemName: symbol).font(.system(size: 11))
                .foregroundStyle(on ? palette.background : palette.text)
                .frame(width: 28, height: 20)
                .background(on ? palette.text : .clear)
                .contentShape(Rectangle())
        }
        .buttonStyle(.plain).help(help).accessibilityLabel(layout.rawValue)
        .accessibilityAddTraits(on ? .isSelected : [])
    }
}
#endif
