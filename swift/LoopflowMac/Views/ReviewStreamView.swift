#if os(macOS)
import AppKit
import Loopflow
import SwiftUI

/// The one diff reader. Changes shows the whole comparison through it; Files'
/// Diff mode shows one file, draft included. Rows, layout, word emphasis,
/// moves and line selection are the same in both.
struct ReviewStream: View {
    @Bindable var store: TaskFilesStore
    let reader: TaskFilesStore.ReviewReader
    let files: [ReviewFile]
    /// Side by side; callers fall back to unified in a narrow pane.
    let split: Bool
    @Environment(\.palette) private var palette

    var body: some View {
        ForEach(files) { file in
            fileHeader(file).id("file:\(file.path)")
            if file.status == .binary {
                ReviewNotice(text: "Binary file · no text to compare")
            } else if file.blocks.isEmpty {
                ReviewNotice(text: file.status == .renamed ? "Renamed without changes" : "No textual changes")
            }
            ForEach(Array(file.blocks.enumerated()), id: \.offset) { _, block in
                blockView(block, file: file).id(Self.blockID(block, path: file.path))
            }
        }
    }

    static func blockID(_ block: ReviewBlock, path: String) -> String {
        switch block {
        case .hunk(let header): "\(path)#\(header)"
        case .context(let line): "\(path)#c\(line.old ?? 0)-\(line.new ?? 0)"
        case .change(let change): change.id
        case .moved(let move): move.id
        }
    }

    private func fileHeader(_ file: ReviewFile) -> some View {
        HStack(spacing: 8) {
            Text(file.path).font(Typography.code(11).weight(.semibold)).lineLimit(1).truncationMode(.middle)
            if let old = file.oldPath { Text("from \(old)").font(Typography.code(10)).foregroundStyle(palette.textSecondary).lineLimit(1) }
            if file.status != .modified { Text(file.status.rawValue).font(.caption2).foregroundStyle(palette.textSecondary) }
            Spacer(minLength: 4)
            Text("+\(file.additions)").foregroundStyle(palette.successTone.ink).font(Typography.code(10))
            Text("−\(file.deletions)").foregroundStyle(palette.failureTone.ink).font(Typography.code(10))
            Button { store.selectFile(file.path, mode: .read) } label: { Image(systemName: "doc.text") }
                .buttonStyle(.plain).help("Read \(file.path) in Files").accessibilityLabel("Read \(file.path) in Files")
        }
        .padding(.horizontal, 10).padding(.vertical, 6)
        .background(palette.surfaceMuted)
        .overlay(alignment: .top) { Divider() }
        .padding(.top, 12)
    }

    @ViewBuilder
    private func blockView(_ block: ReviewBlock, file: ReviewFile) -> some View {
        switch block {
        case .hunk(let header):
            Text(header).font(Typography.code(10)).foregroundStyle(palette.textSecondary)
                .lineLimit(1).truncationMode(.tail)
                .padding(.horizontal, 10).padding(.vertical, 3)
        case .context(let line):
            pairRow(ReviewPair(removed: line, added: line, removedSegments: nil, addedSegments: nil),
                    path: file.path, context: true)
        case .change(let change):
            VStack(spacing: 0) {
                if split {
                    ForEach(Array(change.pairs.enumerated()), id: \.offset) { _, pair in
                        pairRow(pair, path: file.path, context: false)
                    }
                } else {
                    // Unified: every removed line, then every added line.
                    let pairs = Array(change.pairs.enumerated())
                    ForEach(pairs, id: \.offset) { _, pair in
                        if let line = pair.removed { lineRow(line, segments: pair.removedSegments, path: file.path) }
                    }
                    ForEach(pairs, id: \.offset) { _, pair in
                        if let line = pair.added { lineRow(line, segments: pair.addedSegments, path: file.path) }
                    }
                }
            }
            .overlay(alignment: .leading) { currentMark(change.id) }
        case .moved(let move):
            movedView(move, path: file.path)
                .overlay(alignment: .leading) { currentMark(move.id) }
        }
    }

    @ViewBuilder private func currentMark(_ id: String) -> some View {
        if store[place: reader].current == id {
            Rectangle().fill(palette.actionInk).frame(width: 3).accessibilityLabel("Current change")
        }
    }

    // MARK: Moved code

    private func movedView(_ move: ReviewMove, path: String) -> some View {
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
                    Image(systemName: move.end == .source ? "arrow.turn.right.down"
                          : expanded ? "chevron.down" : "chevron.right")
                        .frame(width: 12)
                    Text(Self.movedLabel(move))
                    Spacer(minLength: 0)
                }
                .font(Typography.caption(11))
                .foregroundStyle(palette.textSecondary)
                .padding(.horizontal, 10).padding(.vertical, 4)
                .background(palette.surfaceMuted.opacity(0.6))
                .contentShape(Rectangle())
            }
            .buttonStyle(.plain)
            .accessibilityLabel(Self.movedLabel(move))
            .accessibilityValue(move.end == .destination ? (expanded ? "Expanded" : "Collapsed") : "")
            if expanded, move.end == .destination {
                ForEach(Array(move.pairs.enumerated()), id: \.offset) { _, pair in
                    pairRow(pair, path: path, context: false)
                }
            }
        }
    }

    private static func movedLabel(_ move: ReviewMove) -> String {
        let changed = move.changedLines == 0 ? "unchanged" : "\(move.changedLines) lines changed"
        switch move.end {
        case .destination: return "Moved from L\(move.from) · \(move.lines.count) lines, \(changed)"
        case .source: return "Moved to L\(move.to) · \(move.lines.count) lines"
        }
    }

    // MARK: Lines

    /// Unified: one line with both numbers.
    private func lineRow(_ line: ReviewLine, segments: [ReviewSegment]?, path: String) -> some View {
        let side: ReviewSelection.Side = line.kind == .removed ? .old : .new
        return HStack(spacing: 0) {
            gutter(line.old, side: .old, path: path)
            gutter(line.new, side: .new, path: path)
            cell(line, segments: segments, side: side, path: path)
        }
    }

    /// Side by side: before on the left, after on the right. Context repeats on
    /// both sides; a pure addition or removal leaves the other side empty.
    private func pairRow(_ pair: ReviewPair, path: String, context: Bool) -> some View {
        HStack(spacing: 0) {
            if split || !context {
                HStack(spacing: 0) {
                    gutter(pair.removed?.old, side: .old, path: path)
                    if let line = pair.removed { cell(line, segments: pair.removedSegments, side: .old, path: path) }
                    else { empty }
                }
                Divider()
                HStack(spacing: 0) {
                    gutter(pair.added?.new, side: .new, path: path)
                    if let line = pair.added { cell(line, segments: pair.addedSegments, side: .new, path: path) }
                    else { empty }
                }
            } else if let line = pair.added {
                lineRow(line, segments: nil, path: path)
            }
        }
    }

    private var empty: some View {
        palette.surfaceMuted.opacity(0.4).frame(maxWidth: .infinity, maxHeight: .infinity)
    }

    private func gutter(_ number: Int?, side: ReviewSelection.Side, path: String) -> some View {
        Text(number.map(String.init) ?? "")
            .font(Typography.code(10)).monospacedDigit()
            .foregroundStyle(palette.textSecondary)
            .frame(width: 38, alignment: .trailing)
            .padding(.trailing, 4)
            .contentShape(Rectangle())
            .onTapGesture { if let number { select(number, side: side, path: path) } }
            .help(number == nil ? "" : "Select line; ⇧-click extends")
    }

    private func cell(_ line: ReviewLine, segments: [ReviewSegment]?, side: ReviewSelection.Side, path: String) -> some View {
        let tone: ThemeTone? = switch line.kind {
        case .added: palette.successTone
        case .removed: palette.failureTone
        case .context: nil
        }
        let selected = store.reviewSelection.map {
            $0.contains(line, path: path) && (line.kind == .context || $0.side == side)
        } ?? false
        return HStack(alignment: .top, spacing: 4) {
            // A marker as well as color: + and − read without hue.
            Text(line.kind == .added ? "+" : line.kind == .removed ? "−" : " ")
                .foregroundStyle(tone?.ink ?? palette.textSecondary)
            Text(Self.attributed(line.text, segments: segments, tone: tone))
                .frame(maxWidth: .infinity, alignment: .leading)
                .fixedSize(horizontal: false, vertical: true)
        }
        .font(Typography.code(12))
        .padding(.vertical, 1).padding(.trailing, 8)
        .frame(maxWidth: .infinity, alignment: .leading)
        .background(tone?.fill.opacity(0.55) ?? .clear)
        .background(selected ? palette.actionInk.opacity(0.18) : .clear)
        .textSelection(.enabled)
        .accessibilityElement(children: .combine)
        .accessibilityLabel("\(line.kind == .added ? "Added" : line.kind == .removed ? "Removed" : "Line") \(side == .old ? line.old ?? 0 : line.new ?? 0): \(line.text)")
    }

    /// The changed words of an edited line carry a stronger fill and an underline.
    static func attributed(_ text: String, segments: [ReviewSegment]?, tone: ThemeTone?) -> AttributedString {
        guard let segments, let tone else { return AttributedString(text.isEmpty ? " " : text) }
        var result = AttributedString()
        for segment in segments {
            var part = AttributedString(segment.text)
            if segment.changed {
                part.backgroundColor = tone.ink.opacity(0.28)
                part.underlineStyle = .single
            }
            result.append(part)
        }
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

/// Selected lines: copy them, or put a reference to them into the conversation's unsent input.
struct ReviewSelectionBar: View {
    @Bindable var store: TaskFilesStore
    let selection: ReviewSelection
    /// The diff the selection was made in, for Copy.
    let diff: ReviewDiff
    let addToConversation: ((String) -> Void)?
    @Environment(\.palette) private var palette

    var body: some View {
        HStack(spacing: 10) {
            Text("\(selection.path):\(selection.range) · \(selection.side.rawValue)")
                .font(Typography.code(10)).lineLimit(1).truncationMode(.head)
            Spacer(minLength: 4)
            Button("Copy") {
                NSPasteboard.general.clearContents()
                NSPasteboard.general.setString(selection.text(in: diff), forType: .string)
            }
            Button("Ask in conversation") {
                addToConversation?(selection.reference(revision: store.comparisonDescription ?? store.comparison.title))
            }
            .disabled(addToConversation == nil)
            .help(addToConversation == nil ? "Open the Task conversation to ask about these lines"
                  : "Adds a reference to these lines to the conversation's unsent input")
            .accessibilityIdentifier("changes-ask")
            Button { store.reviewSelection = nil } label: { Image(systemName: "xmark") }
                .buttonStyle(.plain).accessibilityLabel("Clear selection")
        }
        .font(Typography.caption(11))
        .padding(.horizontal, 10).padding(.vertical, 6)
        .background(palette.surfaceMuted)
        .overlay(alignment: .top) { Divider() }
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

/// Unified or side by side; side by side needs a wide pane.
/// Previous/next change buttons and the position, with `[`/`]` while the pane
/// has focus. Changes crosses files in its stream; Files' Diff opens the
/// neighbouring changed file past either end.
struct ReviewStepper: View {
    @Bindable var store: TaskFilesStore
    let reader: TaskFilesStore.ReviewReader
    let isFocused: Bool
    @Environment(\.palette) private var palette

    var body: some View {
        HStack(spacing: 8) {
            Button { go(-1) } label: { Image(systemName: "chevron.up") }
                .help("Previous change ([)").accessibilityLabel("Previous change")
            Button { go(1) } label: { Image(systemName: "chevron.down") }
                .help("Next change (])").accessibilityLabel("Next change")
            Text(store.position(reader)).font(Typography.caption(11)).foregroundStyle(palette.textSecondary)
                .monospacedDigit().accessibilityIdentifier("review-position")
        }
        .background {
            HStack {
                Button("Next change") { go(1) }.keyboardShortcut("]", modifiers: [])
                Button("Previous change") { go(-1) }.keyboardShortcut("[", modifiers: [])
            }
            .disabled(!isFocused).hidden().accessibilityHidden(true).frame(width: 0, height: 0)
        }
    }

    private func go(_ offset: Int) {
        guard let stop = store.step(offset, in: reader) else { return }
        withAnimation(.easeOut(duration: 0.15)) { store[place: reader].anchor = stop.id }
    }
}

extension TaskFilesStore {
    /// The reader's scroll anchor, for `scrollPosition(id:)`.
    func anchor(_ reader: ReviewReader) -> Binding<String?> {
        Binding(get: { self[place: reader].anchor }, set: { self[place: reader].anchor = $0 })
    }
}

struct ReviewLayoutPicker: View {
    @Bindable var store: TaskFilesStore
    let width: CGFloat

    var body: some View {
        Picker("Layout", selection: $store.reviewLayout) {
            ForEach(TaskFilesStore.ReviewLayout.allCases, id: \.self) { Text($0.rawValue).tag($0) }
        }
        .pickerStyle(.menu).labelsHidden().fixedSize()
        .help(width < TaskFilesStore.splitWidth ? "Side by side needs a wider pane" : "Unified or side by side")
    }
}
#endif
