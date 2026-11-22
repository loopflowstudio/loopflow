#if os(macOS)
import AppKit
import Loopflow
import SwiftUI

/// The Diff page: what is compared and how much changed, then the changed
/// files beside the whole comparison as one review stream. The files are in
/// the tour's order when there is one, else by group; the file you are in
/// stays named in a bar above the stream. Comments wait under their lines
/// until you send them to the conversation's unsent input as one message.
struct TaskChangesView: View {
    @Bindable var store: TaskFilesStore
    let task: RoadmapTask?
    let isFocused: Bool
    /// Types into the Task conversation without sending and says whether it
    /// did; nil when no conversation is open.
    let addToConversation: ((String) -> Bool)?
    @Environment(\.palette) private var palette
    /// The pane's width, and the stream's beside the sidebar.
    @State private var paneWidth: CGFloat = 0
    @State private var width: CGFloat = 0
    @State private var showsDelivery = false
    /// Your hidden patterns as `diff_hidden` last read or saved them; nil until read.
    @State private var savedHidden: [String]?
    @State private var hiddenNotice: String?

    private var prURL: URL? { task?.pr?.publication?.github?.url }

    /// Below this the sidebar no longer fits beside the stream and opens over it.
    private static let dockedWidth: CGFloat = 620
    private var docksSidebar: Bool { paneWidth >= Self.dockedWidth }
    private var showsSidebar: Bool { store.review != nil && (store.showsSidebar ?? docksSidebar) }

    var body: some View {
        VStack(spacing: 0) {
            header.fillsPane()
            Divider()
            toolbar.fillsPane()
            Divider()
            ReviewCommentsStrip(store: store, isFocused: isFocused, addToConversation: addToConversation)
            focusStrip
            HStack(spacing: 0) {
                if showsSidebar, docksSidebar {
                    sidebar
                    Divider()
                }
                VStack(spacing: 0) {
                    currentFileBar
                    ScrollView {
                        LazyVStack(alignment: .leading, spacing: 0) {
                            stream
                            footer.id("footer")
                        }
                        .scrollTargetLayout()
                        .fillsPane()
                    }
                    .scrollPosition(id: store.anchor(.changes), anchor: .top)
                    .onScrollPhaseChange { _, phase in if phase == .interacting { store.noteInteraction() } }
                    .onGeometryChange(for: CGFloat.self, of: \.size.width) { width = $0 }
                    // Over the stream, so showing or hiding it moves no line.
                    .overlay(alignment: .top) { enclosingBar }
                    .overlay(alignment: .trailing) { ChangeMarks(store: store, layout: store.layout(width: width), jump: land) }
                }
            }
            .overlay(alignment: .leading) {
                if showsSidebar, !docksSidebar {
                    sidebar
                        .background(palette.background)
                        .overlay(alignment: .trailing) { Divider() }
                        .shadow(color: .black.opacity(0.18), radius: 10, x: 4)
                }
            }
        }
        .background(palette.background)
        .onGeometryChange(for: CGFloat.self, of: \.size.width) { paneWidth = $0 }
        .accessibilityIdentifier("browser-changes")
        .task(id: store.cwd) { await readSettings() }
        .onChange(of: store.yourHidden) { _, patterns in
            guard savedHidden != nil, patterns != savedHidden else { return }
            Task { await saveHidden(patterns) }
        }
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

    // MARK: Header

    var header: some View {
        VStack(alignment: .leading, spacing: 8) {
            // Side by side where both fit; a narrow pane puts the pill under the title.
            ViewThatFits(in: .horizontal) {
                HStack(alignment: .top, spacing: 8) {
                    titles
                    Spacer(minLength: 8)
                    deliveryPill
                }
                VStack(alignment: .leading, spacing: 6) {
                    titles
                    deliveryPill
                }
            }
            if showsDelivery {
                DeliveryCard(store: store.pullRequest, record: task?.pr, taskOpen: task?.isCompleted == false,
                             url: prURL, localHead: store.changes?.headCommit)
                    .clipShape(RoundedRectangle(cornerRadius: 8))
            }
        }
        .padding(.horizontal, 12).padding(.vertical, 8)
    }

    private var titles: some View {
        VStack(alignment: .leading, spacing: 1) {
            Text(title).font(Typography.strong(13))
                .foregroundStyle(store.changesError == nil ? palette.text : palette.warning)
                .lineLimit(2).multilineTextAlignment(.leading)
                .accessibilityIdentifier("changes-title")
            if let compared = store.comparisonDescription {
                Text(compared).font(Typography.caption(11)).foregroundStyle(palette.textSecondary)
                    .lineLimit(1).truncationMode(.middle).help(compared)
                    .accessibilityIdentifier("changes-comparison")
            }
            stepMenu
        }
    }

    /// The whole Task, or one Graph step's commits: the branch's commits since
    /// its base, a run sharing `lf <step>:` read as one step.
    @ViewBuilder private var stepMenu: some View {
        if store.steps.count > 1 || store.step != nil {
            Menu {
                Toggle("Whole Task", isOn: Binding(get: { store.step == nil }, set: { _ in store.showStep(nil) }))
                Divider()
                ForEach(Array(store.steps.enumerated()), id: \.element.id) { index, step in
                    Toggle("\(index + 1). \(step.title)", isOn: Binding(get: { store.step == step }, set: { _ in store.showStep(step) }))
                }
            } label: {
                Text(store.step.map { "Step: \($0.title)" } ?? "Whole Task · \(store.steps.count) steps")
                    .lineLimit(1).truncationMode(.tail)
            }
            .menuStyle(.borderlessButton).fixedSize(horizontal: false, vertical: true)
            .font(Typography.caption(11))
            .help("Compare one Graph step's commits, or the whole Task")
            .accessibilityIdentifier("changes-step")
        }
    }

    // MARK: Hidden paths and groups

    /// Your own settings for this repository, in its `.lf/config.local.yaml`.
    /// Reading through it resolves every layer: built-in, the repository's
    /// `.lf/config.yaml`, your global preferences, then yours here.
    private var personalFile: JSONValue {
        ConfigurationDestination.personalRepository.address(repository: store.cwd, wave: "")
    }

    private func explain(_ key: String) async throws -> ConfigurationValue? {
        let data = try await ServerClient.configuration(.configExplain(destination: personalFile, key: key))
        return try JSONDecoder().decode(ConfigurationExplanation.self, from: data).inspections.first?.settings[key]
    }

    private func readSettings() async {
        // A checkout on another Machine keeps the defaults for this window only.
        guard store.query.remoteMachine == nil else { return }
        do {
            if let groups = try await explain("diff_groups").flatMap({ DiffGroupSetting.parse($0.value) }) {
                store.diffGroups = groups
            }
            if let hidden = try await explain("diff_hidden").flatMap(TaskFilesStore.hidden(from:)) {
                savedHidden = hidden.filter { $0.source == nil }.map(\.pattern)
                store.hidden = hidden
            }
            hiddenNotice = nil
        } catch {
            hiddenNotice = "Diff settings unavailable: \(error.localizedDescription)"
        }
    }

    /// Yours replace your own list; none left removes it, so the layers below
    /// still apply (an empty list would clear them).
    private func saveHidden(_ patterns: [String]) async {
        do {
            if patterns.isEmpty {
                _ = try await ServerClient.configuration(.configUnset(destination: personalFile, key: "diff_hidden"))
            } else {
                let yaml = String(decoding: try JSONEncoder().encode(patterns), as: UTF8.self)
                _ = try await ServerClient.configuration(.configSet(destination: personalFile, key: "diff_hidden", value: yaml))
            }
            savedHidden = patterns
            hiddenNotice = nil
            await readSettings()
        } catch {
            hiddenNotice = "Hidden paths are not saved: \(error.localizedDescription)"
        }
    }

    /// "7 changes in 4 files", or why there is nothing to count. Hidden files,
    /// yours and the tour's, stay in the total: "7 changes in 4 of 9 files".
    private var title: String {
        if let reason = store.changesError { return "Comparison unavailable: \(reason)" }
        guard let changes = store.changes else { return "Reading changes…" }
        guard !changes.files.isEmpty else { return "No changed files" }
        let all = changes.files.count
        guard let review = store.review, let shown = store.shownReview else {
            return all == 1 ? "1 file changed" : "\(all) files changed"
        }
        let stops = shown.stops.count
        let files = shown.files.count == review.files.count
            ? (all == 1 ? "1 file" : "\(all) files") : "\(shown.files.count) of \(review.files.count) files"
        return "\(stops == 1 ? "1 change" : "\(stops) changes") in \(files)"
    }

    /// The delivery headline's first part and a dot for its checks. The dot
    /// has color only while the reading is current.
    private var deliveryPill: some View {
        TimelineView(.periodic(from: .now, by: 10)) { context in
            let pullRequest = store.pullRequest
            let delivery = TaskDelivery.describe(record: task?.pr, taskOpen: task?.isCompleted == false,
                reading: pullRequest.pullRequest, error: pullRequest.error, readAt: pullRequest.readAt,
                localHead: store.changes?.headCommit, now: context.date)
            let checks = pullRequest.pullRequest?.statusCheckRollup ?? []
            let dot: Color = delivery.warning ? palette.warning
                : !delivery.readingIsCurrent ? palette.textTertiary
                : checks.contains { $0.failed } ? palette.failureTone.ink
                : checks.contains { $0.passed } ? palette.successTone.ink : palette.textTertiary
            Button { showsDelivery.toggle() } label: {
                HStack(spacing: 5) {
                    Circle().fill(dot).frame(width: 7, height: 7)
                    Text(delivery.headline.components(separatedBy: " · ").first ?? delivery.headline)
                        .lineLimit(1).truncationMode(.tail)
                    Image(systemName: showsDelivery ? "chevron.up" : "chevron.down")
                        .font(.system(size: 8, weight: .semibold)).foregroundStyle(palette.textSecondary)
                }
                .font(Typography.caption(11))
                .padding(.horizontal, 9).padding(.vertical, 3)
                .frame(maxWidth: 200)
                .background(palette.surface, in: Capsule())
                .overlay { Capsule().strokeBorder(palette.border) }
                .contentShape(Capsule())
            }
            .buttonStyle(.plain)
            .help(delivery.headline)
            .accessibilityIdentifier("changes-delivery-toggle")
            .accessibilityValue(showsDelivery ? "Expanded" : "Collapsed")
        }
        .fixedSize()
    }

    // MARK: Toolbar

    /// The sidebar toggle and stepper always show. The controls after them
    /// take the widest form the pane has room for and are given up in order:
    /// words become symbols, then the layout picker goes (side by side needs
    /// a wide pane anyway), then order and tour, then Focus. What is given up
    /// is in the … menu.
    var toolbar: some View {
        HStack(spacing: 8) {
            Button { store.showsSidebar = !showsSidebar } label: {
                Image(systemName: "sidebar.left").font(.system(size: 12))
                    .foregroundStyle(showsSidebar ? palette.text : palette.textSecondary)
                    .frame(width: 24, height: 20).contentShape(Rectangle())
            }
            .help(showsSidebar ? "Hide the changed files" : "Show the changed files")
            .accessibilityLabel("Changed files")
            .accessibilityIdentifier("changes-sidebar-toggle")
            .accessibilityValue(showsSidebar ? "Expanded" : "Collapsed")
            .disabled(store.review == nil)
            ReviewStepper(store: store, reader: .changes, isFocused: isFocused)
            ViewThatFits(in: .horizontal) {
                controls(compact: false)
                controls(compact: true)
                controls(compact: true, layout: false)
                controls(compact: true, order: false, layout: false)
                Spacer(minLength: 0)
            }
            moreMenu
            // Always its width, so a read starting or ending moves nothing.
            Image(systemName: "arrow.triangle.2.circlepath")
                .foregroundStyle(palette.textSecondary).frame(width: 14)
                .opacity(isReading ? 1 : 0)
                .help(isReading ? reading : "").accessibilityHidden(!isReading)
                .accessibilityLabel(reading)
        }
        .buttonStyle(.plain)
        .font(Typography.caption(11))
        .padding(.horizontal, 10)
        .frame(height: TaskFileChrome.headerHeight + 4)
        .background(palette.surfaceMuted.opacity(0.5))
    }

    private var isReading: Bool { store.isRefreshingChanges || store.unreadShown > 0 }

    /// The title counts what has been read; this says how much has not.
    private var reading: String {
        let unread = store.unreadShown
        return unread == 0 ? "Reading changes…" : unread == 1 ? "Reading 1 more file…" : "Reading \(unread) more files…"
    }

    private func controls(compact: Bool, order: Bool = true, layout: Bool = true) -> some View {
        HStack(spacing: 8) {
            if order {
                orderMenu
                tourButton(compact: compact)
            }
            Spacer(minLength: 4)
            Button { toggleFocus() } label: {
                let on = store.focusedPath != nil
                Group {
                    if compact { Image(systemName: "scope").font(.system(size: 11)) } else { Text("Focus") }
                }
                .foregroundStyle(on ? palette.background : palette.text)
                .padding(.horizontal, compact ? 6 : 9).frame(height: 20)
                .background(on ? palette.text : palette.surface, in: RoundedRectangle(cornerRadius: 6))
                .overlay { RoundedRectangle(cornerRadius: 6).strokeBorder(palette.border) }
                .contentShape(Rectangle())
            }
            .help("Show one file at a time; next and previous still cross files")
            .disabled(store.shownReview?.files.isEmpty ?? true)
            .accessibilityLabel("Focus")
            .accessibilityAddTraits(store.focusedPath != nil ? .isSelected : [])
            if layout { ReviewLayoutPicker(store: store, width: width) }
        }
    }

    @ViewBuilder private var orderChoices: some View {
        ForEach(ReviewOrdering.allCases.filter { $0 != .tour || store.hasTour }, id: \.self) { ordering in
            Toggle(ordering.rawValue, isOn: Binding(get: { store.ordering == ordering }, set: { _ in
                store.orderingChoice = ordering
                store.noteInteraction()
            }))
        }
    }

    /// Tour, By group or By path. Tour is offered when the checkout has one.
    private var orderMenu: some View {
        Menu { orderChoices } label: { Text(store.ordering.rawValue) }
            .menuStyle(.borderlessButton).fixedSize()
            .help(store.hasTour ? "Order of the files. Tour is the checkout's \(DiffTour.path)"
                  : "Order of the files. \(DiffTour.path) in the checkout adds a tour")
            .disabled(store.review == nil)
            .accessibilityIdentifier("changes-order")
    }

    /// Everything after the stepper, for what a narrow pane has no room to lay out.
    private var moreMenu: some View {
        Menu {
            Section("Order") { orderChoices }
            if let action = store.tourAction {
                Button(action) { Task { await store.writeTour() } }
            }
            Toggle("Focus on one file", isOn: Binding(get: { store.focusedPath != nil }, set: { _ in toggleFocus() }))
            Picker("Layout", selection: $store.reviewLayout) {
                ForEach(TaskFilesStore.ReviewLayout.allCases, id: \.self) { layout in
                    Text(layout.rawValue).tag(layout)
                        .selectionDisabled(layout == .split && width < TaskFilesStore.splitWidth)
                }
            }
            Toggle("Wrap long lines", isOn: $store.wrapsLines)
        } label: {
            Image(systemName: "ellipsis.circle").font(.system(size: 12)).frame(width: 24, height: 20)
        }
        .menuStyle(.borderlessButton).menuIndicator(.hidden).fixedSize()
        .help("Order, tour, focus, layout and wrapping")
        .disabled(store.review == nil)
        .accessibilityLabel("Diff options")
        .accessibilityIdentifier("changes-more")
    }

    /// Write tour or Update tour: the `tour` skill rewrites the tour,
    /// and the Diff reads in it once the file lands. The button comes back
    /// beside the spinner once the run has started, in case it dies.
    @ViewBuilder private func tourButton(compact: Bool) -> some View {
        if store.tourWriting.isUnderway {
            ProgressView().controlSize(.mini).help(store.tourStatus ?? "")
                .accessibilityLabel("Writing the tour")
        }
        if let action = store.tourAction {
            Button { Task { await store.writeTour() } } label: {
                if compact { Image(systemName: "signpost.right").accessibilityLabel(action) } else { Text(action).lineLimit(1) }
            }
                .buttonStyle(WorkPlainButtonStyle())
                .help("Run the tour skill in the Task's checkout to write \(DiffTour.path)")
                .disabled(store.review == nil)
                .accessibilityIdentifier("changes-write-tour")
        }
    }

    /// The file whose lines are at the top of the stream: the focused one,
    /// else the one the reading anchor or the current change is in. While a
    /// file's own header is the top row, the file above it is still the one
    /// you are in, so the bar never names the header under it.
    private var currentPath: String? {
        if let focused = store.focusedPath { return focused }
        let files = store.shownReview?.files ?? []
        let place = store[place: .changes]
        guard let id = place.anchor ?? place.current else { return files.first?.path }
        if id.hasPrefix("file:") {
            let index = files.firstIndex { $0.path == id.dropFirst("file:".count) } ?? 0
            return files.isEmpty ? nil : files[max(0, index - 1)].path
        }
        if id == "footer" { return files.last?.path }
        return files.first { id.hasPrefix($0.path + "#") }?.path ?? files.first?.path
    }

    /// The header of the file you are in, fixed above the stream while its
    /// lines scroll under it. It is the first file's only header.
    @ViewBuilder private var currentFileBar: some View {
        if let currentPath, let file = store.shownReview?.file(currentPath) {
            let group = store.shownGroups.first { group in group.isTour && group.files.contains { $0.path == file.path } }
            ReviewFileTitle(store: store, file: file, note: group?.title ?? file.oldPath.map { "from \($0)" }, viewable: true)
                .fillsPane()
                .frame(height: TaskFileChrome.headerHeight)
                .background(palette.surfaceMuted)
                .contextMenu {
                    Button("Open in Files") { store.selectFile(file.path, mode: .diff) }
                    Button("Read file") { store.selectFile(file.path, mode: .read) }
                }
                .accessibilityElement(children: .contain)
                .accessibilityIdentifier("changes-current-file")
            Divider()
        }
    }

    /// The function, type or heading the top line sits in, under the file's
    /// name, while the file's own lines are at the top.
    @ViewBuilder private var enclosingBar: some View {
        if let path = currentPath, let file = store.shownReview?.file(path),
           let id = store[place: .changes].anchor, id.hasPrefix(path + "#"),
           let declaration = store.opened(file).enclosing(id) {
            Text(declaration).font(Typography.code(11)).foregroundStyle(palette.textSecondary)
                .lineLimit(1).truncationMode(.tail)
                .padding(.leading, ReviewStream.gutterWidth + 18).padding(.trailing, 12).padding(.vertical, 3)
                .fillsPane()
                .background(palette.background.opacity(0.94))
                .overlay(alignment: .bottom) { Divider() }
                .help(declaration)
                .allowsHitTesting(false)
                .accessibilityIdentifier("changes-enclosing")
        }
    }

    /// Land on a change a mark beside the scrollbar names.
    private func land(_ mark: ReviewMark) {
        if store.focusedPath != nil { store.focusedPath = mark.path }
        withAnimation(.easeOut(duration: 0.15)) { store[place: .changes] = .init(anchor: mark.id, current: mark.id) }
        store.noteInteraction()
    }

    private func toggleFocus() {
        if store.focusedPath != nil {
            store.focusedPath = nil
        } else {
            let current = store[place: .changes].current.flatMap { id in store.shownReview?.stops.first { $0.id == id }?.path }
            store.focusedPath = current ?? currentPath
        }
        store.noteInteraction()
    }

    @ViewBuilder private var focusStrip: some View {
        if let focused = store.focusedPath, let review = store.shownReview {
            HStack(spacing: 6) {
                Text("Only \((focused as NSString).lastPathComponent)").font(Typography.strong(11))
                Text(review.files.count == 2 ? "· 1 more file" : "· \(review.files.count - 1) more files")
                    .font(Typography.caption(11)).foregroundStyle(palette.textSecondary)
                Spacer(minLength: 4)
                Button { toggleFocus() } label: { Image(systemName: "xmark").font(.system(size: 9, weight: .semibold)) }
                    .buttonStyle(.plain).help("Show all files").accessibilityLabel("Show all files")
            }
            .lineLimit(1)
            .padding(.horizontal, 12).padding(.vertical, 5)
            .background(palette.surfaceElement)
            .help(focused)
            Divider()
        }
    }

    private var sidebar: some View {
        ChangedFilesSidebar(store: store, current: currentPath, notice: hiddenNotice, jump: jump)
            .frame(width: ChangedFilesSidebar.width)
    }

    /// Bring a file's first lines to the top of the stream, under its name in the bar.
    private func jump(to path: String) {
        if store.focusedPath != nil { store.focusedPath = path }
        store[place: .changes] = .init(anchor: store.shownReview?.file(path)?.topID,
                                       current: store.shownReview?.stops.first { $0.path == path }?.id)
        // Over a narrow stream the sidebar gives the room back.
        if !docksSidebar { store.showsSidebar = nil }
        store.noteInteraction()
    }

    // MARK: Stream

    @ViewBuilder private var stream: some View {
        if let review = store.shownReview {
            let files = store.focusedPath.map { path in review.files.filter { $0.path == path } } ?? review.files
            if let tour = store.tourStatus {
                ReviewNotice(text: tour)
            }
            if review.files.isEmpty, store.review?.files.isEmpty == false {
                ReviewNotice(text: "Every changed file is hidden or filtered out · the changed files list what is hidden")
            }
            ReviewStream(store: store, reader: .changes, files: files, split: store.layout(width: width) == .split,
                         headings: headings, headsFirstFile: false, addToConversation: addToConversation)
        } else if store.changesError == nil {
            ProgressView().frame(maxWidth: .infinity).padding(24)
        }
    }

    /// The tour's reason, above the first file it heads.
    private var headings: [String: String] {
        var headings: [String: String] = [:]
        for group in store.shownGroups where group.isTour && !group.title.isEmpty {
            if let first = group.files.first { headings[first.path] = group.title }
        }
        return headings
    }

    // MARK: Footer

    /// The stream says where it ends, and offers the way back.
    @ViewBuilder private var footer: some View {
        VStack(spacing: 6) {
            if let first = store.shownReview?.stops.first, store.focusedPath == nil {
                Text("End of changes").font(Typography.caption(11)).foregroundStyle(palette.textSecondary)
                Button("Back to the first change") {
                    store[place: .changes] = .init(anchor: store.shownReview?.file(first.path)?.topID, current: first.id)
                    store.noteInteraction()
                }
                .buttonStyle(WorkPlainButtonStyle()).font(.caption)
            }
            if store.query.remoteMachine == nil, let directory = store.changes?.recoveryDirectory {
                Button("Open saved versions ↗") { NSWorkspace.shared.open(URL(fileURLWithPath: directory)) }
                    .buttonStyle(WorkPlainButtonStyle()).font(.caption)
            }
        }
        .frame(maxWidth: .infinity)
        .padding(.vertical, 24)
    }
}

/// A quiet mark beside the scrollbar for every change in the stream, at its
/// place counted in rows. Marks within a few points of a tick merge into it,
/// so a long diff reads as ticks rather than a bar; only the current change takes the
/// accent, and a commented file adds a dot. Click one to land on its change.
struct ChangeMarks: View {
    let store: TaskFilesStore
    let layout: TaskFilesStore.ReviewLayout
    let jump: (ReviewMark) -> Void
    @Environment(\.palette) private var palette

    /// Marks closer than this to a tick, in points, draw as that tick.
    static let merge: CGFloat = 6

    var body: some View {
        if let review = store.shownReview {
            let files = store.focusedPath.map { path in review.files.filter { $0.path == path } } ?? review.files
            let marks = ReviewDiff(files: files).marks(layout: layout, collapsed: store.isViewed)
            let commented = Set(store.comments.map(\.selection.path))
            let current = store[place: .changes].current
            GeometryReader { proxy in
                let height = max(proxy.size.height - 8, 0)
                ForEach(Self.merged(marks, height: height), id: \.first.id) { group in
                    let isCurrent = group.marks.contains { $0.id == current }
                    Button { jump(group.first) } label: {
                        RoundedRectangle(cornerRadius: 1)
                            .fill(isCurrent ? palette.accent : palette.textTertiary.opacity(0.55))
                            .frame(width: 4, height: 3)
                            .frame(width: 10)
                            .overlay(alignment: .topLeading) {
                                if group.marks.contains(where: { commented.contains($0.path) }) {
                                    Circle().fill(palette.accent).frame(width: 3, height: 3)
                                }
                            }
                            .contentShape(Rectangle())
                    }
                    .buttonStyle(.plain)
                    .help((group.first.path as NSString).lastPathComponent)
                    .position(x: proxy.size.width - 5, y: 4 + group.y)
                }
            }
            .frame(width: 10)
            .accessibilityHidden(true)
        }
    }

    /// One tick: the marks it stands for, in stream order, and where it sits.
    struct Group {
        let marks: [ReviewMark]
        let y: CGFloat
        var first: ReviewMark { marks[0] }
    }

    /// Marks in stream order, each joining the tick above it when it lands
    /// within `merge` points of it.
    static func merged(_ marks: [ReviewMark], height: CGFloat) -> [Group] {
        var groups: [Group] = []
        for mark in marks {
            let y = mark.position * height
            if let group = groups.last, y - group.y < merge {
                groups[groups.count - 1] = Group(marks: group.marks + [mark], y: group.y)
            } else {
                groups.append(Group(marks: [mark], y: y))
            }
        }
        return groups
    }
}

private extension View {
    /// Exactly the width the pane offers, whatever the content asks for, with
    /// the content at the leading edge. A row that takes its content's width
    /// is centered in the column and moves when the content changes; one that
    /// asks for more than the pane widens the column past it.
    func fillsPane() -> some View {
        frame(minWidth: 0, maxWidth: .infinity, alignment: .leading)
    }
}

/// The changed files beside the stream, in its order and under its headings.
/// Type to narrow the stream and click a file to bring it to the top. Below
/// them, what the tour hid for this change with its reasons, and the path
/// text you always hide in this repository.
private struct ChangedFilesSidebar: View {
    static let width: CGFloat = 236

    @Bindable var store: TaskFilesStore
    /// The file at the top of the stream.
    let current: String?
    /// Why the hidden paths last failed to read or save.
    let notice: String?
    let jump: (String) -> Void
    @Environment(\.palette) private var palette
    @State private var hiding = ""

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            TextField("Filter files", text: $store.reviewFilter)
                .textFieldStyle(.roundedBorder).font(Typography.caption(12))
                .padding(8)
                .accessibilityIdentifier("changes-filter")
            Divider()
            ScrollView {
                LazyVStack(alignment: .leading, spacing: 0) {
                    if store.viewedCount > 0, let shown = store.shownReview?.files.count {
                        Text("\(store.viewedCount) / \(shown) viewed").foregroundStyle(palette.textSecondary)
                            .padding(.horizontal, 8).padding(.top, 8)
                            .accessibilityIdentifier("changes-viewed-count")
                    }
                    ForEach(store.shownGroups) { group in
                        groupRow(group)
                        ForEach(group.files) { file in fileRow(file, in: group) }
                    }
                    if store.shownGroups.isEmpty {
                        Text("No files to show").foregroundStyle(palette.textSecondary).padding(8)
                    }
                    tourHidden
                    alwaysHidden
                }
                .padding(.bottom, 12)
            }
        }
        .font(Typography.caption(11))
        .frame(maxHeight: .infinity, alignment: .top)
        .background(palette.surfaceMuted.opacity(0.35))
        .accessibilityIdentifier("changes-files")
    }

    private func heading(_ text: String) -> some View {
        Text(text).font(Typography.strong(11)).foregroundStyle(palette.textSecondary)
            .padding(.horizontal, 8).padding(.top, 12).padding(.bottom, 3)
    }

    @ViewBuilder private func groupRow(_ group: ReviewGroup) -> some View {
        if !group.title.isEmpty {
            HStack(alignment: .firstTextBaseline, spacing: 6) {
                // A tour's reason reads quieter than the files under it.
                Text(group.title).font(group.isTour ? Typography.caption(11) : Typography.strong(11))
                    .foregroundStyle(palette.textSecondary)
                    .lineLimit(group.isTour ? 2 : 1).truncationMode(.tail)
                    .help(group.title)
                Spacer(minLength: 4)
                if group.id.hasPrefix("path:"), group.title.hasSuffix("/") {
                    Button("Hide") { store.hide("/" + group.title) }.buttonStyle(WorkPlainButtonStyle())
                        .help("Always leave \(group.title) out of the Diff")
                }
            }
            .padding(.horizontal, 8).padding(.top, 10).padding(.bottom, 3)
        }
    }

    private func fileRow(_ file: ReviewFile, in group: ReviewGroup) -> some View {
        let folder = group.id.hasPrefix("path:") && group.title.hasSuffix("/") ? group.title.count : nil
        let comments = store.comments.count { $0.selection.path == file.path }
        let viewed = store.isViewed(file)
        return Button { jump(file.path) } label: {
            HStack(spacing: 6) {
                Text(folder.map { String(file.path.dropFirst($0)) } ?? (file.path as NSString).lastPathComponent)
                    .font(Typography.code(11)).lineLimit(1).truncationMode(.head)
                    .foregroundStyle(viewed ? palette.textTertiary : palette.text)
                if viewed {
                    Image(systemName: "checkmark").font(.system(size: 8, weight: .bold))
                        .foregroundStyle(palette.textTertiary).accessibilityLabel("Viewed")
                }
                Spacer(minLength: 4)
                if comments > 0 {
                    Label("\(comments)", systemImage: "text.bubble").labelStyle(.titleAndIcon)
                        .foregroundStyle(palette.accent)
                        .accessibilityLabel(comments == 1 ? "1 comment" : "\(comments) comments")
                }
                if !store.isReading(file) { counts(added: file.additions, removed: file.deletions) }
            }
            .padding(.horizontal, 8).padding(.vertical, 3)
            .background(file.path == current ? palette.surfaceElement : .clear)
            .contentShape(Rectangle())
        }
        .buttonStyle(.plain).help(file.path)
        .accessibilityAddTraits(file.path == current ? .isSelected : [])
    }

    private func counts(added: Int, removed: Int) -> some View {
        HStack(spacing: 4) {
            Text("+\(added)").foregroundStyle(palette.successTone.ink)
            Text("−\(removed)").foregroundStyle(palette.failureTone.ink)
        }
        .font(Typography.code(10))
    }

    private func fileCount(_ count: Int) -> String { count == 1 ? "1 file" : "\(count) files" }

    /// Paths the tour left out of this change, each with its reason and one
    /// click to read it anyway. They never join your own hidden paths.
    @ViewBuilder private var tourHidden: some View {
        let files = store.review?.files ?? []
        let entries = (store.diffTour?.hide ?? []).filter { entry in files.contains { entry.matches($0.path) } }
        if !entries.isEmpty {
            heading("Hidden by the tour for this change")
            ForEach(entries, id: \.path) { entry in
                let shown = store.shownTourHides.contains(entry.path)
                VStack(alignment: .leading, spacing: 1) {
                    HStack(spacing: 6) {
                        Text(entry.path).font(Typography.code(11)).lineLimit(1).truncationMode(.head)
                        Text(fileCount(files.count { entry.matches($0.path) })).foregroundStyle(palette.textSecondary)
                        Spacer(minLength: 4)
                        Button(shown ? "Hide" : "Show") {
                            if shown { store.shownTourHides.remove(entry.path) } else { store.shownTourHides.insert(entry.path) }
                        }
                        .buttonStyle(WorkPlainButtonStyle())
                    }
                    if !entry.why.isEmpty {
                        Text(entry.why).foregroundStyle(palette.textSecondary).fixedSize(horizontal: false, vertical: true)
                    }
                }
                .padding(.horizontal, 8).padding(.vertical, 3)
            }
        }
    }

    /// What is always left out: yours, each removable, and the repository's
    /// and the defaults, each named by where it comes from. A field adds yours.
    @ViewBuilder private var alwaysHidden: some View {
        let files = store.review?.files ?? []
        let hidden = files.count { store.hides($0.path) }
        heading("Always hidden")
        ForEach(store.hidden, id: \.pattern) { entry in
            HStack(spacing: 6) {
                Text(entry.pattern).font(Typography.code(11)).lineLimit(1).truncationMode(.head)
                Text(fileCount(files.count { DiffPattern.matches(entry.pattern, $0.path) }))
                    .foregroundStyle(palette.textSecondary)
                Spacer(minLength: 4)
                if let source = entry.source {
                    Text(source).foregroundStyle(palette.textSecondary)
                        .help("From \(source == "Default" ? "Loopflow's default" : "the \(source.lowercased()) settings"); diff_hidden there changes it")
                } else {
                    Button { store.unhide(entry.pattern) } label: {
                        Image(systemName: "xmark").font(.system(size: 8, weight: .bold)).frame(width: 16, height: 16)
                            .contentShape(Rectangle())
                    }
                    .buttonStyle(.plain).help("Stop hiding paths matching \(entry.pattern)")
                    .accessibilityLabel("Stop hiding \(entry.pattern)")
                }
            }
            .padding(.horizontal, 8).padding(.vertical, 2)
        }
        TextField("tests/, *.lock…", text: $hiding)
            .textFieldStyle(.roundedBorder).font(Typography.code(11))
            .onSubmit { store.hide(hiding); hiding = "" }
            .help("Hide every changed file whose path contains this text, or ends with it after *")
            .padding(.horizontal, 8).padding(.vertical, 4)
            .accessibilityIdentifier("changes-hide")
        if hidden > 0 {
            Toggle(hidden == 1 ? "Show 1 hidden file" : "Show \(hidden) hidden files", isOn: $store.showsHidden)
                .toggleStyle(.checkbox).controlSize(.small)
                .padding(.horizontal, 8)
        }
        if let notice {
            Text(notice).foregroundStyle(palette.warning).textSelection(.enabled)
                .fixedSize(horizontal: false, vertical: true).padding(8)
        }
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
