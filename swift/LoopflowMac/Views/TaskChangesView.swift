#if os(macOS)
import AppKit
import Loopflow
import SwiftUI

/// The Changes page: where the work is delivered, then the whole comparison as
/// one review stream. Next/previous crosses files; selecting lines puts a
/// reference to them into the conversation's unsent input.
struct TaskChangesView: View {
    @Bindable var store: TaskFilesStore
    let task: RoadmapTask?
    let isFocused: Bool
    /// Types into the Task conversation without sending; nil when no conversation is open.
    let addToConversation: ((String) -> Void)?
    @Environment(\.palette) private var palette
    @State private var width: CGFloat = 0

    private var prURL: URL? { task?.pr?.publication?.github?.url }
    private var split: Bool { store.reviewLayout == .split && width >= 640 }

    var body: some View {
        VStack(spacing: 0) {
            toolbar
            Divider()
            ScrollView {
                LazyVStack(alignment: .leading, spacing: 0) {
                    DeliveryCard(store: store.pullRequest, record: task?.pr, taskOpen: task?.isCompleted == false,
                                 url: prURL, localHead: store.changes?.headCommit)
                        .id("delivery")
                    changedFiles.id("files")
                    stream
                    footer.id("footer")
                }
                .scrollTargetLayout()
            }
            .scrollPosition(id: $store.reviewAnchor, anchor: .top)
            .onScrollPhaseChange { _, phase in if phase == .interacting { store.noteInteraction() } }
            .onGeometryChange(for: CGFloat.self, of: \.size.width) { width = $0 }
            if let selection = store.reviewSelection { selectionBar(selection) }
        }
        .background(palette.background)
        .background { shortcuts }
        .accessibilityIdentifier("browser-changes")
        .task(id: "\(store.comparison)|\(task?.reference.workspace?.baseCommit ?? "")") {
            store.observeFiles()
            await store.loadReview()
        }
        .task(id: prURL) {
            // Fresh while you look at it; nothing polls a hidden page.
            while !Task.isCancelled {
                await store.pullRequest.refresh(prURL)
                guard prURL != nil else { return }
                do { try await Task.sleep(for: .seconds(60)) } catch { return }
            }
        }
    }

    // MARK: Toolbar

    private var position: String {
        guard let review = store.review else { return "" }
        let stops = review.stops
        guard !stops.isEmpty else { return "No changes" }
        guard let current = store.currentStop, let index = stops.firstIndex(where: { $0.id == current }) else {
            return "\(stops.count) changes"
        }
        return "\(index + 1) of \(stops.count)"
    }

    private var toolbar: some View {
        HStack(spacing: 8) {
            Button { go(-1) } label: { Image(systemName: "chevron.up") }
                .help("Previous change ([)").accessibilityLabel("Previous change")
            Button { go(1) } label: { Image(systemName: "chevron.down") }
                .help("Next change (])").accessibilityLabel("Next change")
            Text(position).font(Typography.caption(11)).foregroundStyle(palette.textSecondary)
                .monospacedDigit().accessibilityIdentifier("changes-position")
            if let path = store.focusedPath {
                Text(path).font(Typography.code(10)).lineLimit(1).truncationMode(.head).help(path)
            }
            Spacer(minLength: 4)
            Button(store.focusedPath == nil ? "Focus file" : "All files") { toggleFocus() }
                .help("Show one file at a time; next and previous still cross files")
                .disabled(store.review?.files.isEmpty ?? true)
            Picker("Layout", selection: $store.reviewLayout) {
                ForEach(TaskFilesStore.ReviewLayout.allCases, id: \.self) { Text($0.rawValue).tag($0) }
            }
            .pickerStyle(.menu).labelsHidden().fixedSize()
            .help(width < 640 ? "Side by side needs a wider pane" : "Unified or side by side")
            if store.reviewError != nil || store.isRefreshingChanges {
                Image(systemName: store.reviewError != nil ? "exclamationmark.triangle" : "arrow.triangle.2.circlepath")
                    .foregroundStyle(store.reviewError != nil ? palette.warning : palette.textSecondary)
                    .help(store.reviewError ?? "Reading changes…")
            }
        }
        .buttonStyle(.plain)
        .font(Typography.caption(11))
        .padding(.horizontal, 10)
        .frame(height: TaskFileChrome.headerHeight)
    }

    private var shortcuts: some View {
        HStack {
            Button("Next change") { go(1) }.keyboardShortcut("]", modifiers: [])
            Button("Previous change") { go(-1) }.keyboardShortcut("[", modifiers: [])
        }
        .disabled(!isFocused).hidden().accessibilityHidden(true).frame(width: 0, height: 0)
    }

    private func go(_ offset: Int) {
        guard let stop = store.step(offset) else { return }
        withAnimation(.easeOut(duration: 0.15)) { store.reviewAnchor = stop.id }
    }

    private func toggleFocus() {
        if store.focusedPath != nil {
            store.focusedPath = nil
        } else {
            let current = store.currentStop.flatMap { id in store.review?.stops.first { $0.id == id }?.path }
            store.focusedPath = current ?? store.review?.files.first?.path
        }
        store.noteInteraction()
    }

    // MARK: Changed files

    private var changedFiles: some View {
        VStack(alignment: .leading, spacing: 2) {
            if let reason = store.changesError {
                Text("Comparison unavailable: \(reason)").font(.caption).foregroundStyle(palette.warning)
            } else if let changes = store.changes {
                Text(changes.files.isEmpty ? "No changed files" : "\(changes.files.count) changed files")
                    .font(Typography.strong(12)).padding(.bottom, 4)
                ForEach(changes.files, id: \.path) { file in
                    let reviewed = store.review?.file(file.path)
                    Button { jump(to: file.path) } label: {
                        HStack(spacing: 8) {
                            Text(file.path).font(Typography.code(11)).lineLimit(1).truncationMode(.middle)
                            if store.documents[file.path]?.dirty == true { Text("•").help("Unsaved draft") }
                            Spacer(minLength: 4)
                            if let reviewed {
                                if reviewed.status == .binary { Text("binary").foregroundStyle(palette.textSecondary) }
                                Text("+\(reviewed.additions)").foregroundStyle(palette.successTone.ink)
                                Text("−\(reviewed.deletions)").foregroundStyle(palette.failureTone.ink)
                            }
                        }
                        .font(Typography.code(10))
                        .padding(.horizontal, 4).padding(.vertical, 2)
                        .workSelection(store.focusedPath == file.path)
                        .contentShape(Rectangle())
                    }
                    .buttonStyle(.plain).help(file.path)
                    .contextMenu {
                        Button("Open in Files") { store.selectFile(file.path, mode: .diff) }
                        Button("Read file") { store.selectFile(file.path, mode: .read) }
                    }
                }
            } else {
                ProgressView().controlSize(.small)
            }
        }
        .padding(.horizontal, 14).padding(.vertical, 10)
    }

    private func jump(to path: String) {
        if store.focusedPath != nil { store.focusedPath = path }
        store.currentStop = store.review?.stops.first { $0.path == path }?.id
        store.reviewAnchor = "file:\(path)"
        store.noteInteraction()
    }

    // MARK: Stream

    @ViewBuilder private var stream: some View {
        if let review = store.review {
            let files = store.focusedPath.map { path in review.files.filter { $0.path == path } } ?? review.files
            if store.reviewTruncated {
                notice("Comparison truncated at 1 MB · later files are missing from the stream")
            }
            if let error = store.reviewError {
                notice("Showing the last reading · \(error)")
            }
            if let focused = store.focusedPath {
                notice("Showing \(focused) · \(review.files.count - 1) other files hidden")
            }
            ForEach(files) { file in
                fileHeader(file).id("file:\(file.path)")
                if file.status == .binary {
                    notice("Binary file · no text to compare")
                } else if file.blocks.isEmpty {
                    notice(file.status == .renamed ? "Renamed without changes" : "No textual changes")
                }
                ForEach(Array(file.blocks.enumerated()), id: \.offset) { _, block in
                    blockView(block, file: file).id(blockID(block, path: file.path))
                }
            }
        } else if let error = store.reviewError {
            ContentUnavailableView("Comparison unavailable", systemImage: "doc", description: Text(error))
                .padding(.vertical, 24)
        } else {
            ProgressView().frame(maxWidth: .infinity).padding(24)
        }
    }

    private func notice(_ text: String) -> some View {
        Text(text).font(.caption).foregroundStyle(palette.textSecondary)
            .padding(.horizontal, 14).padding(.vertical, 6)
    }

    private func blockID(_ block: ReviewBlock, path: String) -> String {
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
                .buttonStyle(.plain).help("Open \(file.path) in Files").accessibilityLabel("Open \(file.path) in Files")
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
        if store.currentStop == id {
            Rectangle().fill(palette.actionInk).frame(width: 3).accessibilityLabel("Current change")
        }
    }

    // MARK: Moved code

    private func movedView(_ move: ReviewMove, path: String) -> some View {
        let expanded = store.expandedMoves.contains(move.id)
        return VStack(alignment: .leading, spacing: 0) {
            Button {
                if move.end == .source {
                    store.currentStop = move.counterpart
                    store.reviewAnchor = move.counterpart
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
                    Text(movedLabel(move))
                    Spacer(minLength: 0)
                }
                .font(Typography.caption(11))
                .foregroundStyle(palette.textSecondary)
                .padding(.horizontal, 10).padding(.vertical, 4)
                .background(palette.surfaceMuted.opacity(0.6))
                .contentShape(Rectangle())
            }
            .buttonStyle(.plain)
            .accessibilityLabel(movedLabel(move))
            .accessibilityValue(move.end == .destination ? (expanded ? "Expanded" : "Collapsed") : "")
            if expanded, move.end == .destination {
                ForEach(Array(move.pairs.enumerated()), id: \.offset) { _, pair in
                    pairRow(pair, path: path, context: false)
                }
            }
        }
    }

    private func movedLabel(_ move: ReviewMove) -> String {
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
            gutter(line.old, side: .old, line: line, path: path)
            gutter(line.new, side: .new, line: line, path: path)
            cell(line, segments: segments, side: side, path: path)
        }
    }

    /// Side by side: before on the left, after on the right. Context repeats on
    /// both sides; a pure addition or removal leaves the other side empty.
    private func pairRow(_ pair: ReviewPair, path: String, context: Bool) -> some View {
        HStack(spacing: 0) {
            if split || !context {
                HStack(spacing: 0) {
                    gutter(pair.removed?.old, side: .old, line: pair.removed, path: path)
                    if let line = pair.removed { cell(line, segments: pair.removedSegments, side: .old, path: path) }
                    else { empty }
                }
                Divider()
                HStack(spacing: 0) {
                    gutter(pair.added?.new, side: .new, line: pair.added, path: path)
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

    private func gutter(_ number: Int?, side: ReviewSelection.Side, line: ReviewLine?, path: String) -> some View {
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

    // MARK: Selection

    private func select(_ number: Int, side: ReviewSelection.Side, path: String) {
        let extend = NSEvent.modifierFlags.contains(.shift)
        if extend, let current = store.reviewSelection, current.path == path, current.side == side {
            store.reviewSelection = current.extended(to: number)
        } else {
            store.reviewSelection = ReviewSelection(path: path, side: side, first: number, last: number)
        }
        store.noteInteraction()
    }

    private func selectionBar(_ selection: ReviewSelection) -> some View {
        HStack(spacing: 10) {
            Text("\(selection.path):\(selection.range) · \(selection.side.rawValue)")
                .font(Typography.code(10)).lineLimit(1).truncationMode(.head)
            Spacer(minLength: 4)
            Button("Copy") {
                guard let review = store.review else { return }
                NSPasteboard.general.clearContents()
                NSPasteboard.general.setString(selection.text(in: review), forType: .string)
            }
            Button("Ask in conversation") {
                addToConversation?(selection.reference(revision: store.reviewRevision))
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

    // MARK: Footer

    @ViewBuilder private var footer: some View {
        VStack(alignment: .leading, spacing: 4) {
            if let sha = store.changes?.baseCommit {
                Text("\(store.comparison.title) · parent \(sha.prefix(10))")
                    .font(Typography.code(10)).foregroundStyle(palette.textSecondary)
            }
            if store.query.remoteMachine == nil, let directory = store.changes?.recoveryDirectory {
                Button("Open saved versions ↗") { NSWorkspace.shared.open(URL(fileURLWithPath: directory)) }
                    .buttonStyle(WorkPlainButtonStyle()).font(.caption)
            }
        }
        .padding(14)
    }
}

/// Loopflow's record of the Task's PR, then GitHub's reading with its age.
private struct DeliveryCard: View {
    let store: PullRequestStore
    let record: PrSnapshot?
    let taskOpen: Bool
    let url: URL?
    let localHead: String?
    @Environment(\.palette) private var palette
    @State private var showsChecks = false
    @State private var showsDescription = false

    var body: some View {
        TimelineView(.periodic(from: .now, by: 10)) { context in
            let delivery = TaskDelivery.describe(record: record, taskOpen: taskOpen, reading: store.pullRequest,
                error: store.error, readAt: store.readAt, localHead: localHead, now: context.date)
            VStack(alignment: .leading, spacing: 6) {
                HStack(alignment: .firstTextBaseline, spacing: 8) {
                    Image(systemName: record?.publication?.github == nil ? "arrow.triangle.branch" : "arrow.triangle.pull")
                        .foregroundStyle(palette.textSecondary)
                    Text(delivery.headline).font(Typography.strong(12))
                        .foregroundStyle(delivery.warning ? palette.warning : palette.text)
                        .fixedSize(horizontal: false, vertical: true)
                        .accessibilityIdentifier("changes-delivery")
                    Spacer(minLength: 4)
                    if let url {
                        Button { Task { await store.refresh(url) } } label: { Image(systemName: "arrow.clockwise") }
                            .buttonStyle(.plain).disabled(store.refreshing)
                            .help("Read GitHub again").accessibilityLabel("Refresh pull request")
                        Link("Open on GitHub ↗", destination: url).font(.caption)
                    }
                }
                if let detail = delivery.detail {
                    Text(detail).font(.caption).foregroundStyle(palette.textSecondary)
                }
                if let pr = store.pullRequest {
                    reading(pr, current: delivery.readingIsCurrent)
                }
            }
        }
        .padding(.horizontal, 14).padding(.vertical, 10)
        .frame(maxWidth: .infinity, alignment: .leading)
        .background(palette.surfaceMuted.opacity(0.5))
        .textSelection(.enabled)
    }

    /// A reading that is not current stays readable but loses its colors.
    private func reading(_ pr: GitHubPullRequest, current: Bool) -> some View {
        VStack(alignment: .leading, spacing: 4) {
            Text(pr.title).font(Typography.body(12)).lineLimit(2)
            if let checks = pr.statusCheckRollup, !checks.isEmpty {
                DisclosureGroup(isExpanded: $showsChecks) {
                    ForEach(Array(checks.enumerated()), id: \.offset) { _, check in
                        HStack(spacing: 6) {
                            Image(systemName: check.failed ? "xmark.circle" : check.passed ? "checkmark.circle" : "clock")
                                .foregroundStyle(!current ? palette.textSecondary
                                                 : check.failed ? palette.failureTone.ink
                                                 : check.passed ? palette.successTone.ink : palette.textSecondary)
                            if let link = check.link { Link(check.label, destination: link) } else { Text(check.label) }
                            Spacer(minLength: 0)
                            Text(check.result.replacingOccurrences(of: "_", with: " ").capitalized)
                                .foregroundStyle(palette.textSecondary)
                        }.font(.caption)
                    }
                } label: { Text(pr.checksSummary).font(.caption) }
            }
            if !pr.reviewRequests.isEmpty {
                Text("Waiting on \(pr.reviewRequests.map(\.label).joined(separator: ", "))")
                    .font(.caption).foregroundStyle(palette.textSecondary)
            }
            if !pr.body.isEmpty {
                DisclosureGroup("Description", isExpanded: $showsDescription) {
                    MarkdownBlocks(source: pr.body).equatable()
                }.font(.caption)
            }
        }
        .opacity(current ? 1 : 0.6)
    }
}
#endif
