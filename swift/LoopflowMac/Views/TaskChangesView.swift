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
    @State private var hiddenNotice: Failure?

    private var prURL: URL? { task?.pr?.publication?.github?.url }

    /// Below this the sidebar no longer fits beside the stream and opens over it.
    private var docksSidebar: Bool { paneWidth >= TaskFileChrome.dockedWidth }
    private var showsSidebar: Bool { store.diffReview.review != nil && (store.diffReview.showsSidebar ?? docksSidebar) }

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
                    .scrollPosition(id: store.diffReview.anchor(.changes), anchor: .top)
                    .onGeometryChange(for: CGFloat.self, of: \.size.width) { width = $0 }
                    // Over the stream, so showing or hiding it moves no line.
                    .overlay(alignment: .top) { EnclosingDeclarationBar(store: store) }
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
        .onChange(of: store.diffReview.yourHidden) { _, patterns in
            guard savedHidden != nil, patterns != savedHidden else { return }
            Task { await saveHidden(patterns) }
        }
        .task(id: "\(store.diffReview.comparison)|\(task?.reference.workspace?.baseCommit ?? "")") {
            store.observeFiles()
            await store.diffReview.loadReview()
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
        VStack(alignment: .leading, spacing: Spacing.sm) {
            // Side by side where both fit; a narrow pane puts the pill under the title.
            ViewThatFits(in: .horizontal) {
                HStack(alignment: .top, spacing: Spacing.sm) {
                    titles
                    Spacer(minLength: 8)
                    deliveryPill
                }
                VStack(alignment: .leading, spacing: Spacing.xs) {
                    titles
                    deliveryPill
                }
            }
            if showsDelivery {
                DeliveryCard(store: store.pullRequest, record: task?.pr, taskOpen: task?.isCompleted == false,
                             url: prURL, localHead: store.workingChanges?.headCommit, runsOn: store.machineName)
                    .clipShape(RoundedRectangle(cornerRadius: CornerRadius.row))
            }
        }
        .padding(.horizontal, Spacing.md).padding(.vertical, Spacing.sm)
    }

    private var titles: some View {
        VStack(alignment: .leading, spacing: Spacing.xxs) {
            HStack(alignment: .firstTextBaseline, spacing: Spacing.sm) {
                Group {
                    if let reason = store.diffReview.reviewReading.error {
                        FailureLine("Comparison unavailable.", machine: reason)
                    } else {
                        Text(store.diffReview.changesTitle).font(Typography.textStrong).foregroundStyle(palette.text)
                            .fixedSize(horizontal: false, vertical: true).multilineTextAlignment(.leading)
                    }
                }
                .accessibilityIdentifier("changes-title")
                stepMenu
            }
            if let compared = store.diffReview.comparisonLine {
                Text(Self.names(compared)).font(Typography.text).foregroundStyle(palette.textSecondary)
                    .fixedSize(horizontal: false, vertical: true)
                    .accessibilityIdentifier("changes-comparison")
            }
        }
    }

    /// Runs marked as code (a ref, a commit) in mono; the rest is the sentence.
    static func names(_ line: AttributedString) -> AttributedString {
        var line = line
        for run in line.runs where run.inlinePresentationIntent?.contains(.code) == true {
            line[run.range].inlinePresentationIntent = nil
            line[run.range].font = Typography.mono
        }
        return line
    }

    /// The whole Task, or one Graph step's commits: the branch's commits since
    /// its base, a run sharing `lf <step>:` read as one step.
    @ViewBuilder private var stepMenu: some View {
        if store.diffReview.steps.count > 1 || store.diffReview.step != nil {
            Menu {
                Toggle("Whole Task", isOn: Binding(get: { store.diffReview.step == nil }, set: { _ in store.diffReview.showStep(nil) }))
                Divider()
                ForEach(Array(store.diffReview.steps.enumerated()), id: \.element.id) { index, step in
                    Toggle("\(index + 1). \(step.title)", isOn: Binding(get: { store.diffReview.step == step }, set: { _ in store.diffReview.showStep(step) }))
                }
            } label: {
                Text(store.diffReview.step.map { "Step: \($0.title)" } ?? "Whole Task, \(store.diffReview.steps.count) steps")
            }
            .menuStyle(.borderlessButton).fixedSize(horizontal: false, vertical: true)
            .font(Typography.text)
            .help("Compare one Graph step's commits, or the whole Task")
            .accessibilityIdentifier("changes-step")
        }
    }

    // MARK: Hidden paths and groups

    /// Your own settings for this repository, in its `.lf/config.local.yaml`.
    private var personalFile: JSONValue {
        .object(["kind": .string(ConfigurationDestination.personalRepository.rawValue), "repo": .string(store.cwd)])
    }

    private func readSettings() async {
        // A checkout on another Machine keeps the defaults for this window only.
        guard store.machine == nil else { return }
        do {
            struct Inspection: Decodable { let settings: [String: ConfigurationValue] }
            let work = ServedRead.configSettings(directory: store.cwd, wave: nil, skill: nil, graph: nil, step: nil)
            let settings = try await ServerClient.read(work).decode([Inspection].self).first?.settings
            if let groups = settings?["diff_groups"].flatMap({ DiffGroupSetting.parse($0.value) }) {
                store.diffReview.diffGroups = groups
            }
            if let hidden = settings?["diff_hidden"].flatMap(DiffReviewState.hidden(from:)) {
                savedHidden = hidden.filter { $0.source == nil }.map(\.pattern)
                store.diffReview.hidden = hidden
            }
            hiddenNotice = nil
        } catch {
            hiddenNotice = Failure(what: "Diff settings did not load.", machine: error.localizedDescription)
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
            hiddenNotice = Failure(what: "Hidden paths were not saved.", machine: error.localizedDescription)
        }
    }

    /// The delivery headline's first part and a dot for its checks. The dot
    /// has color only while the reading is current.
    private var deliveryPill: some View {
        TimelineView(.periodic(from: .now, by: 10)) { context in
            let pullRequest = store.pullRequest
            let delivery = TaskDelivery.describe(record: task?.pr, taskOpen: task?.isCompleted == false,
                reading: pullRequest.pullRequest, error: pullRequest.error, readAt: pullRequest.readAt,
                localHead: store.workingChanges?.headCommit, runsOn: store.machineName, now: context.date)
            let checks = pullRequest.pullRequest?.statusCheckRollup ?? []
            let dot: Color = delivery.warning ? palette.warning
                : !delivery.readingIsCurrent ? palette.textTertiary
                : checks.contains { $0.failed } ? palette.failureTone.ink
                : checks.contains { $0.passed } ? palette.successTone.ink : palette.textTertiary
            Button { showsDelivery.toggle() } label: {
                HStack(spacing: Spacing.xs) {
                    Circle().fill(dot).frame(width: 7, height: 7)
                    Text(delivery.headline.components(separatedBy: " · ").first ?? delivery.headline)
                        .fixedSize(horizontal: false, vertical: true).multilineTextAlignment(.leading)
                    Image(systemName: showsDelivery ? "chevron.up" : "chevron.down")
                        .font(Typography.meta).fontWeight(.semibold).foregroundStyle(palette.textSecondary)
                }
                .font(Typography.text)
                .padding(.horizontal, Spacing.sm).padding(.vertical, Spacing.xxs)
                .frame(minHeight: HitTarget.minimum)
                .background(palette.surface, in: Capsule())
                .overlay { Capsule().strokeBorder(palette.border) }
                .contentShape(Capsule())
            }
            .buttonStyle(.plain)
            .help(delivery.headline)
            .accessibilityIdentifier("changes-delivery-toggle")
            .accessibilityValue(showsDelivery ? "Expanded" : "Collapsed")
        }
        .fixedSize(horizontal: false, vertical: true)
    }

    // MARK: Toolbar

    /// The sidebar toggle and stepper always show. The controls after them
    /// take the widest form the pane has room for and are given up in order:
    /// words become symbols, then the layout picker goes (side by side needs
    /// a wide pane anyway), then order and tour, then Focus. What is given up
    /// is in the … menu.
    var toolbar: some View {
        HStack(spacing: Spacing.sm) {
            Button { store.diffReview.showsSidebar = !showsSidebar } label: {
                Image(systemName: "sidebar.left").font(Typography.text)
                    .foregroundStyle(showsSidebar ? palette.text : palette.textSecondary)
                    .frame(width: 24, height: 20).contentShape(Rectangle())
            }
            .help(showsSidebar ? "Hide the changed files" : "Show the changed files")
            .accessibilityLabel("Changed files")
            .accessibilityIdentifier("changes-sidebar-toggle")
            .accessibilityValue(showsSidebar ? "Expanded" : "Collapsed")
            .disabled(store.diffReview.review == nil)
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
        .font(Typography.meta)
        .padding(.horizontal, Spacing.sm)
        .frame(height: TaskFileChrome.headerHeight + 4)
        .background(palette.surfaceMuted.opacity(0.5))
    }

    private var isReading: Bool { store.diffReview.reviewReading.loading || store.diffReview.unreadShown > 0 }

    /// The title counts what has been read; this says how much has not.
    private var reading: String {
        let unread = store.diffReview.unreadShown
        return unread == 0 ? "Reading changes…" : unread == 1 ? "Reading 1 more file…" : "Reading \(unread) more files…"
    }

    private func controls(compact: Bool, order: Bool = true, layout: Bool = true) -> some View {
        HStack(spacing: Spacing.sm) {
            if order {
                orderMenu
                tourButton(compact: compact)
            }
            Spacer(minLength: 4)
            Button { toggleFocus() } label: {
                let on = store.diffReview.focusedPath != nil
                Group {
                    if compact { Image(systemName: "scope").font(Typography.meta) } else { Text("Focus") }
                }
                .foregroundStyle(on ? palette.background : palette.text)
                .padding(.horizontal, compact ? 6 : 9).frame(height: 20)
                .background(on ? palette.text : palette.surface, in: RoundedRectangle(cornerRadius: CornerRadius.control))
                .overlay { RoundedRectangle(cornerRadius: CornerRadius.control).strokeBorder(palette.border) }
                .contentShape(Rectangle())
            }
            .help("Show one file at a time; next and previous still cross files")
            .disabled(store.diffReview.shownReview?.files.isEmpty ?? true)
            .accessibilityLabel("Focus")
            .accessibilityAddTraits(store.diffReview.focusedPath != nil ? .isSelected : [])
            if layout { ReviewLayoutPicker(store: store, width: width) }
        }
    }

    @ViewBuilder private var orderChoices: some View {
        ForEach(ReviewOrdering.allCases.filter { $0 != .tour || store.diffReview.hasTour }, id: \.self) { ordering in
            Toggle(ordering.rawValue, isOn: Binding(get: { store.diffReview.ordering == ordering }, set: { _ in
                store.diffReview.orderingChoice = ordering
            }))
        }
    }

    /// Tour, By group or By path. Tour is offered when the checkout has one.
    private var orderMenu: some View {
        Menu { orderChoices } label: { Text(store.diffReview.ordering.rawValue) }
            .menuStyle(.borderlessButton).fixedSize()
            .help(store.diffReview.hasTour ? "Order of the files. Tour is the checkout's \(DiffTour.path)"
                  : "Order of the files. \(DiffTour.path) in the checkout adds a tour")
            .disabled(store.diffReview.review == nil)
            .accessibilityIdentifier("changes-order")
    }

    /// Everything after the stepper, for what a narrow pane has no room to lay out.
    private var moreMenu: some View {
        Menu {
            Section("Order") { orderChoices }
            if let action = store.diffReview.tourAction {
                Button(action) { Task { await store.diffReview.writeTour() } }
            }
            Toggle("Focus on one file", isOn: Binding(get: { store.diffReview.focusedPath != nil }, set: { _ in toggleFocus() }))
            Picker("Layout", selection: Bindable(store.diffReview).reviewLayout) {
                ForEach(DiffReviewState.ReviewLayout.allCases, id: \.self) { layout in
                    Text(layout.rawValue).tag(layout)
                        .selectionDisabled(layout == .split && width < DiffReviewState.splitWidth)
                }
            }
            Toggle("Wrap long lines", isOn: Bindable(store.diffReview).wrapsLines)
        } label: {
            Image(systemName: "ellipsis.circle").font(Typography.text).frame(width: 24, height: 20)
        }
        .menuStyle(.borderlessButton).menuIndicator(.hidden).fixedSize()
        .help("Order, tour, focus, layout and wrapping")
        .disabled(store.diffReview.review == nil)
        .accessibilityLabel("Diff options")
        .accessibilityIdentifier("changes-more")
    }

    /// Write tour or Update tour: the `tour` skill rewrites the tour,
    /// and the Diff reads in it once the file lands. The button comes back
    /// beside the spinner once the run has started, in case it dies.
    @ViewBuilder private func tourButton(compact: Bool) -> some View {
        if store.diffReview.tourWriting.isUnderway {
            ProgressView().controlSize(.mini).help(store.diffReview.tourStatus ?? "")
                .accessibilityLabel("Writing the tour")
        }
        if let action = store.diffReview.tourAction {
            Button { Task { await store.diffReview.writeTour() } } label: {
                if compact { Image(systemName: "signpost.right").accessibilityLabel(action) } else { Text(action).lineLimit(1) }
            }
                .buttonStyle(WorkPlainButtonStyle())
                .help("Run the tour skill in the Task's checkout to write \(DiffTour.path)")
                .disabled(store.diffReview.review == nil)
                .accessibilityIdentifier("changes-write-tour")
        }
    }

    /// The header of the file you are in, fixed above the stream while its
    /// lines scroll under it. It is the first file's only header.
    @ViewBuilder private var currentFileBar: some View {
        if let path = store.diffReview.topPath, let file = store.diffReview.shownReview?.file(path) {
            let group = store.diffReview.shownGroups.first { group in group.isTour && group.files.contains { $0.path == file.path } }
            ReviewFileTitle(store: store, file: file, note: group?.title ?? file.oldPath.map { "from \($0)" }, viewable: true)
                .fillsPane()
                .padding(.vertical, Spacing.xs)
                .frame(minHeight: TaskFileChrome.headerHeight)
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

    /// Every shown file's changes, viewed ones included.
    private var shownStops: [ReviewStop] { store.diffReview.shownReview.map { store.diffReview.stops(of: $0.files, in: .changes) } ?? [] }

    private func toggleFocus() {
        if store.diffReview.focusedPath != nil {
            store.diffReview.focusedPath = nil
        } else {
            let current = store.diffReview.current(.changes).flatMap { id in shownStops.first { $0.id == id }?.path }
            store.diffReview.focusedPath = current ?? store.diffReview.topPath
        }
    }

    @ViewBuilder private var focusStrip: some View {
        if let focused = store.diffReview.focusedPath, let review = store.diffReview.shownReview {
            HStack(spacing: Spacing.xs) {
                Text("Only \((focused as NSString).lastPathComponent)").font(Typography.textStrong)
                Text(review.files.count == 2 ? "· 1 more file" : "· \(review.files.count - 1) more files")
                    .font(Typography.meta).foregroundStyle(palette.textSecondary)
                Spacer(minLength: 4)
                Button { toggleFocus() } label: { Image(systemName: "xmark").font(Typography.meta).fontWeight(.semibold) }
                    .buttonStyle(.plain).help("Show all files").accessibilityLabel("Show all files")
            }
            .lineLimit(1)
            .padding(.horizontal, Spacing.md).padding(.vertical, Spacing.xs)
            .background(palette.surfaceElement)
            .help(focused)
            Divider()
        }
    }

    private var sidebar: some View {
        ChangedFilesSidebar(store: store, current: store.diffReview.topPath, notice: hiddenNotice, jump: jump)
            .frame(width: ChangedFilesSidebar.width)
    }

    /// Bring a file's first lines to the top of the stream, under its name in the bar.
    private func jump(to path: String) {
        if store.diffReview.focusedPath != nil { store.diffReview.focusedPath = path }
        store.diffReview[place: .changes] = .init(anchor: store.diffReview.shownReview?.file(path).map { store.diffReview.topID(of: $0, in: .changes) },
                                       current: shownStops.first { $0.path == path }?.id)
        // Over a narrow stream the sidebar gives the room back.
        if !docksSidebar { store.diffReview.showsSidebar = nil }
    }

    // MARK: Stream

    /// The files in the window, or the focused one. Above the window, the way
    /// to the files before it; at its end, a row that draws the next ones
    /// once it scrolls into view.
    @ViewBuilder private var stream: some View {
        if let review = store.diffReview.shownReview {
            let window = store.diffReview.focusedPath == nil ? store.diffReview.window : review.files.indices
            let files = store.diffReview.focusedPath.map { path in review.files.filter { $0.path == path } }
                ?? Array(review.files[window])
            if let tour = store.diffReview.tourStatus {
                ReviewNotice(text: tour)
            }
            if review.files.isEmpty, store.diffReview.review?.files.isEmpty == false {
                ReviewNotice(text: "Every changed file is hidden or filtered out. The changed files list what is hidden.")
            }
            if window.lowerBound > 0 {
                Button(window.lowerBound == 1 ? "1 file above · Show" : "\(window.lowerBound) files above · Show") {
                    store.diffReview.showEarlierFiles()
                }
                .buttonStyle(WorkPlainButtonStyle()).font(Typography.meta)
                .padding(.horizontal, Spacing.md).padding(.vertical, Spacing.sm)
                .accessibilityIdentifier("changes-files-above")
            }
            ReviewStream(store: store, reader: .changes, files: files, split: store.diffReview.layout(width: width) == .split,
                         headings: headings, headsFirstFile: false, addToConversation: addToConversation)
            if window.upperBound < review.files.count {
                let more = review.files.count - window.upperBound
                ReviewNotice(text: more == 1 ? "1 more file" : "\(more) more files")
                    .id("window-end-\(window.upperBound)")
                    .onAppear { store.diffReview.extendWindow() }
            }
        } else if store.diffReview.reviewReading.error == nil {
            ProgressView().frame(maxWidth: .infinity).padding(Spacing.xxl)
        }
    }

    /// The tour's reason, above the first file it heads.
    private var headings: [String: String] {
        var headings: [String: String] = [:]
        for group in store.diffReview.shownGroups where group.isTour && !group.title.isEmpty {
            if let first = group.files.first { headings[first.path] = group.title }
        }
        return headings
    }

    // MARK: Footer

    /// The stream says where it ends, and offers the way back.
    @ViewBuilder private var footer: some View {
        VStack(spacing: Spacing.xs) {
            if let first = shownStops.first, store.diffReview.focusedPath == nil,
               store.diffReview.window.upperBound == store.diffReview.shownReview?.files.count {
                Text("End of changes.").font(Typography.meta).foregroundStyle(palette.textSecondary)
                Button("Back to the first change") {
                    store.diffReview[place: .changes] = .init(anchor: store.diffReview.shownReview?.file(first.path).map { store.diffReview.topID(of: $0, in: .changes) },
                                                   current: first.id)
                }
                .buttonStyle(WorkPlainButtonStyle()).font(Typography.meta)
            }
            if store.machine == nil, let directory = store.diffReview.changes?.recoveryDirectory {
                Button("Open saved versions") { NSWorkspace.shared.open(URL(fileURLWithPath: directory)) }
                    .buttonStyle(WorkPlainButtonStyle()).font(Typography.meta)
            }
        }
        .frame(maxWidth: .infinity)
        .padding(.vertical, Spacing.xxl)
    }
}

/// The function, type or heading the top line sits in, under the file's
/// name, while the file's own lines are at the top. Its own view, so the row
/// each scroll lands on redraws this line and nothing else.
private struct EnclosingDeclarationBar: View {
    let store: TaskFilesStore
    @Environment(\.palette) private var palette

    var body: some View {
        if let path = store.diffReview.topPath, let file = store.diffReview.shownReview?.file(path),
           let id = store.diffReview[place: .changes].anchor, id.hasPrefix(path + "#"),
           let declaration = store.diffReview.opened(file).enclosing(id) {
            Text(declaration).font(Typography.mono).foregroundStyle(palette.textSecondary)
                .lineLimit(1).truncationMode(.tail)
                .padding(.leading, ReviewStream.gutterWidth + 18).padding(.trailing, Spacing.md).padding(.vertical, Spacing.xxs)
                .fillsPane()
                .background(palette.background.opacity(0.94))
                .overlay(alignment: .bottom) { Divider() }
                .help(declaration)
                .allowsHitTesting(false)
                .accessibilityIdentifier("changes-enclosing")
        }
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
/// A file's added and removed line counts, colored like the Diff.
struct LineCounts: View {
    let added: Int
    let removed: Int
    @Environment(\.palette) private var palette

    var body: some View {
        HStack(spacing: Spacing.xs) {
            Text("+\(added)").foregroundStyle(palette.successTone.ink)
            Text("−\(removed)").foregroundStyle(palette.failureTone.ink)
        }
        .font(Typography.mono)
    }
}

struct ChangedFilesSidebar: View {
    static let width: CGFloat = 236

    @Bindable var store: TaskFilesStore
    /// The file at the top of the stream.
    let current: String?
    /// The read or save of the hidden paths that last failed.
    let notice: Failure?
    let jump: (String) -> Void
    @Environment(\.palette) private var palette
    @State private var hiding = ""

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            TextField("Filter files", text: Bindable(store.diffReview).reviewFilter)
                .textFieldStyle(.roundedBorder).font(Typography.text)
                .padding(Spacing.sm)
                .accessibilityIdentifier("changes-filter")
            Divider()
            ScrollView {
                LazyVStack(alignment: .leading, spacing: 0) {
                    if let viewed = store.diffReview.viewedSummary {
                        Text(viewed).foregroundStyle(palette.textSecondary)
                            .padding(.horizontal, Spacing.sm).padding(.top, Spacing.sm)
                            .accessibilityIdentifier("changes-viewed-count")
                    }
                    ForEach(store.diffReview.shownGroups) { group in
                        groupRow(group)
                        ForEach(group.files) { file in fileRow(file, in: group) }
                    }
                    if store.diffReview.shownGroups.isEmpty {
                        Text("No files to show.").emptyLine().padding(Spacing.sm)
                    }
                    tourHidden
                    alwaysHidden
                }
                .padding(.bottom, Spacing.md)
            }
        }
        .font(Typography.meta)
        .frame(maxHeight: .infinity, alignment: .top)
        .background(palette.surfaceMuted.opacity(0.35))
        .accessibilityIdentifier("changes-files")
    }

    private func heading(_ text: String) -> some View {
        Text(text).font(Typography.textStrong).foregroundStyle(palette.textSecondary)
            .padding(.horizontal, Spacing.sm).padding(.top, Spacing.md).padding(.bottom, Spacing.xxs)
    }

    @ViewBuilder private func groupRow(_ group: ReviewGroup) -> some View {
        if !group.title.isEmpty {
            HStack(alignment: .firstTextBaseline, spacing: Spacing.xs) {
                // A tour's reason reads quieter than the files under it.
                Text(group.title).font(group.isTour ? Typography.meta : Typography.textStrong)
                    .foregroundStyle(palette.textSecondary)
                    .lineLimit(group.isTour ? 2 : 1).truncationMode(.tail)
                    .help(group.title)
                Spacer(minLength: 4)
                if group.id.hasPrefix("path:"), group.title.hasSuffix("/") {
                    Button("Hide") { store.diffReview.hide("/" + group.title) }.buttonStyle(WorkPlainButtonStyle())
                        .help("Always leave \(group.title) out of the Diff")
                }
            }
            .padding(.horizontal, Spacing.sm).padding(.top, Spacing.sm).padding(.bottom, Spacing.xxs)
        }
    }

    private func fileRow(_ file: ReviewFile, in group: ReviewGroup) -> some View {
        let folder = group.id.hasPrefix("path:") && group.title.hasSuffix("/") ? group.title.count : nil
        let place = Self.place(file.path, under: folder)
        let comments = store.comments.items.count { $0.selection.path == file.path && $0.original.source.revision == store.diffReview.revision }
        let viewed = store.diffReview.isViewed(file)
        return Button { jump(file.path) } label: {
            VStack(alignment: .leading, spacing: Spacing.xxs) {
                HStack(alignment: .firstTextBaseline, spacing: Spacing.xs) {
                    Text(place.name).font(Typography.mono)
                        .foregroundStyle(viewed ? palette.textTertiary : palette.text)
                        .fixedSize(horizontal: false, vertical: true)
                    Spacer(minLength: 4)
                    if comments > 0 {
                        Label("\(comments)", systemImage: "text.bubble").labelStyle(.titleAndIcon)
                            .foregroundStyle(palette.accent)
                            .accessibilityLabel(comments == 1 ? "1 comment" : "\(comments) comments")
                    }
                    if !store.diffReview.isReading(file), file.status != .binary { LineCounts(added: file.additions, removed: file.deletions).fixedSize() }
                }
                if place.directory != nil || viewed {
                    HStack(alignment: .firstTextBaseline, spacing: Spacing.xs) {
                        if let directory = place.directory {
                            Text(directory).font(Typography.mono).foregroundStyle(palette.textSecondary)
                                .fixedSize(horizontal: false, vertical: true)
                        }
                        if viewed {
                            Label("Viewed", systemImage: "checkmark").labelStyle(.titleAndIcon)
                                .foregroundStyle(palette.textTertiary)
                        }
                    }
                }
            }
            .padding(.horizontal, Spacing.sm).padding(.vertical, Spacing.xs)
            .frame(maxWidth: .infinity, alignment: .leading)
            .background(file.path == current ? palette.surfaceElement : .clear)
            .contentShape(Rectangle())
        }
        .buttonStyle(.plain).help(file.path)
        .accessibilityAddTraits(file.path == current ? .isSelected : [])
    }

    /// A file's name, then its directory under the group's folder, if any.
    static func place(_ path: String, under folder: Int?) -> (name: String, directory: String?) {
        let relative = folder.map { String(path.dropFirst($0)) } ?? path
        let directory = (relative as NSString).deletingLastPathComponent
        return ((relative as NSString).lastPathComponent, directory.isEmpty ? nil : directory + "/")
    }

    private func fileCount(_ count: Int) -> String { count == 1 ? "1 file" : "\(count) files" }

    /// Paths the tour left out of this change, each with its reason and one
    /// click to read it anyway. They never join your own hidden paths.
    @ViewBuilder private var tourHidden: some View {
        let files = store.diffReview.review?.files ?? []
        let entries = (store.diffReview.diffTour?.hide ?? []).filter { entry in files.contains { entry.matches($0.path) } }
        if !entries.isEmpty {
            heading("Hidden by the tour for this change")
            ForEach(entries, id: \.path) { entry in
                let shown = store.diffReview.shownTourHides.contains(entry.path)
                VStack(alignment: .leading, spacing: Spacing.xxs) {
                    HStack(spacing: Spacing.xs) {
                        Text(entry.path).font(Typography.mono).lineLimit(1).truncationMode(.head)
                        Text(fileCount(files.count { entry.matches($0.path) })).foregroundStyle(palette.textSecondary)
                        Spacer(minLength: 4)
                        Button(shown ? "Hide" : "Show") {
                            if shown { store.diffReview.shownTourHides.remove(entry.path) } else { store.diffReview.shownTourHides.insert(entry.path) }
                        }
                        .buttonStyle(WorkPlainButtonStyle())
                    }
                    if !entry.why.isEmpty {
                        Text(entry.why).foregroundStyle(palette.textSecondary).fixedSize(horizontal: false, vertical: true)
                    }
                }
                .padding(.horizontal, Spacing.sm).padding(.vertical, Spacing.xxs)
            }
        }
    }

    /// What is always left out: yours, each removable, and the repository's
    /// and the defaults, each named by where it comes from. A field adds yours.
    @ViewBuilder private var alwaysHidden: some View {
        let files = store.diffReview.review?.files ?? []
        let hidden = files.count { store.diffReview.hides($0.path) }
        heading("Always hidden")
        ForEach(store.diffReview.hidden, id: \.pattern) { entry in
            HStack(spacing: Spacing.xs) {
                Text(entry.pattern).font(Typography.mono).lineLimit(1).truncationMode(.head)
                Text(fileCount(files.count { DiffPattern.matches(entry.pattern, $0.path) }))
                    .foregroundStyle(palette.textSecondary)
                Spacer(minLength: 4)
                if let source = entry.source {
                    Text(source).foregroundStyle(palette.textSecondary)
                        .help("From \(source == "Default" ? "Loopflow's default" : "the \(source.lowercased()) settings"); diff_hidden there changes it")
                } else {
                    Button { store.diffReview.unhide(entry.pattern) } label: {
                        Image(systemName: "xmark").font(Typography.meta).fontWeight(.bold).frame(width: 16, height: 16)
                            .contentShape(Rectangle())
                    }
                    .buttonStyle(.plain).help("Stop hiding paths matching \(entry.pattern)")
                    .accessibilityLabel("Stop hiding \(entry.pattern)")
                }
            }
            .padding(.horizontal, Spacing.sm).padding(.vertical, Spacing.xxs)
        }
        TextField("tests/, *.lock…", text: $hiding)
            .textFieldStyle(.roundedBorder).font(Typography.mono)
            .onSubmit { store.diffReview.hide(hiding); hiding = "" }
            .help("Hide every changed file whose path contains this text, or ends with it after *")
            .padding(.horizontal, Spacing.sm).padding(.vertical, Spacing.xs)
            .accessibilityIdentifier("changes-hide")
        if hidden > 0 {
            Toggle(hidden == 1 ? "Show 1 hidden file" : "Show \(hidden) hidden files", isOn: Bindable(store.diffReview).showsHidden)
                .toggleStyle(.checkbox).controlSize(.small)
                .padding(.horizontal, Spacing.sm)
        }
        if let notice {
            FailureLine(notice).padding(Spacing.sm)
        }
    }
}

/// Loopflow's record of the Task's PR, then GitHub's reading with its age.
struct DeliveryCard: View {
    let store: PullRequestStore
    let record: PrSnapshot?
    let taskOpen: Bool
    let url: URL?
    let localHead: String?
    let runsOn: String?
    @Environment(\.palette) private var palette
    @State private var showsChecks = false
    @State private var showsDescription = false

    var body: some View {
        TimelineView(.periodic(from: .now, by: 10)) { context in
            let delivery = TaskDelivery.describe(record: record, taskOpen: taskOpen, reading: store.pullRequest,
                error: store.error, readAt: store.readAt, localHead: localHead, runsOn: runsOn, now: context.date)
            VStack(alignment: .leading, spacing: Spacing.xs) {
                HStack(alignment: .firstTextBaseline, spacing: Spacing.sm) {
                    Image(systemName: record?.publication?.github == nil ? "arrow.triangle.branch" : "arrow.triangle.pull")
                        .foregroundStyle(palette.textSecondary)
                    Text(delivery.headline).font(Typography.textStrong)
                        .foregroundStyle(delivery.warning ? palette.warning : palette.text)
                        .fixedSize(horizontal: false, vertical: true)
                        .accessibilityIdentifier("changes-delivery")
                }
                if let detail = delivery.detail {
                    Text(detail.hasSuffix(".") ? detail : detail + ".").font(Typography.text).foregroundStyle(palette.textSecondary)
                }
                if let pr = store.pullRequest {
                    reading(pr, current: delivery.readingIsCurrent)
                }
                if let url {
                    HStack(spacing: Spacing.sm) {
                        Button("Read again") { Task { await store.refresh(url) } }
                            .disabled(store.refreshing)
                            .help("Read GitHub again").accessibilityLabel("Refresh pull request")
                        Button("Open on GitHub") { NSWorkspace.shared.open(url) }
                            .help(url.absoluteString)
                            .accessibilityIdentifier("changes-open-github")
                    }
                    .buttonStyle(LoopflowButtonStyle())
                }
            }
        }
        .padding(.horizontal, Spacing.md).padding(.vertical, Spacing.sm)
        .frame(maxWidth: .infinity, alignment: .leading)
        .background(palette.surfaceMuted.opacity(0.5))
        .textSelection(.enabled)
    }

    /// A reading that is not current stays readable but loses its colors.
    private func reading(_ pr: GitHubPullRequest, current: Bool) -> some View {
        VStack(alignment: .leading, spacing: Spacing.xs) {
            Text(pr.title).font(Typography.text).fixedSize(horizontal: false, vertical: true)
            if let checks = pr.statusCheckRollup, !checks.isEmpty {
                DisclosureGroup(isExpanded: $showsChecks) {
                    ForEach(Array(checks.enumerated()), id: \.offset) { _, check in
                        HStack(spacing: Spacing.xs) {
                            Image(systemName: check.failed ? "xmark.circle" : check.passed ? "checkmark.circle" : "clock")
                                .foregroundStyle(!current ? palette.textSecondary
                                                 : check.failed ? palette.failureTone.ink
                                                 : check.passed ? palette.successTone.ink : palette.textSecondary)
                            Group {
                                if let link = check.link { Link(check.label, destination: link) } else { Text(check.label) }
                            }
                            .font(Typography.mono)
                            Spacer(minLength: 0)
                            Text(check.result.replacingOccurrences(of: "_", with: " ").capitalized)
                                .foregroundStyle(palette.textSecondary).fixedSize()
                        }.font(Typography.text)
                    }
                } label: { Text(pr.checksSummary).font(Typography.text) }
            }
            if !pr.reviewRequests.isEmpty {
                Text("Waiting on \(pr.reviewRequests.map(\.label).joined(separator: ", ")).")
                    .font(Typography.text).foregroundStyle(palette.textSecondary)
            }
            if !pr.body.isEmpty {
                DisclosureGroup("Description", isExpanded: $showsDescription) {
                    MarkdownBlocks(source: pr.body).equatable()
                }.font(Typography.text)
            }
        }
        .opacity(current ? 1 : 0.6)
    }
}
#endif
