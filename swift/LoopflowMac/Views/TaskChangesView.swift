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
            .scrollPosition(id: store.anchor(.changes), anchor: .top)
            .onScrollPhaseChange { _, phase in if phase == .interacting { store.noteInteraction() } }
            .onGeometryChange(for: CGFloat.self, of: \.size.width) { width = $0 }
            if let selection = store.reviewSelection, let review = store.review {
                ReviewSelectionBar(store: store, selection: selection, diff: review, addToConversation: addToConversation)
            }
        }
        .background(palette.background)
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

    private var toolbar: some View {
        HStack(spacing: 8) {
            ReviewStepper(store: store, reader: .changes, isFocused: isFocused)
            if let path = store.focusedPath {
                Text(path).font(Typography.code(10)).lineLimit(1).truncationMode(.head).help(path)
            }
            Spacer(minLength: 4)
            Button(store.focusedPath == nil ? "Focus file" : "All files") { toggleFocus() }
                .help("Show one file at a time; next and previous still cross files")
                .disabled(store.review?.files.isEmpty ?? true)
            ReviewLayoutPicker(store: store, width: width)
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

    private func toggleFocus() {
        if store.focusedPath != nil {
            store.focusedPath = nil
        } else {
            let current = store[place: .changes].current.flatMap { id in store.review?.stops.first { $0.id == id }?.path }
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
        store[place: .changes] = .init(anchor: "file:\(path)", current: store.review?.stops.first { $0.path == path }?.id)
        store.noteInteraction()
    }

    // MARK: Stream

    @ViewBuilder private var stream: some View {
        if let review = store.review {
            let files = store.focusedPath.map { path in review.files.filter { $0.path == path } } ?? review.files
            if store.reviewTruncated {
                ReviewNotice(text: "Comparison truncated at 1 MB · later files are missing from the stream")
            }
            if let error = store.reviewError {
                ReviewNotice(text: "Showing the last reading · \(error)")
            }
            if let focused = store.focusedPath {
                ReviewNotice(text: "Showing \(focused) · \(review.files.count - 1) other files hidden")
            }
            ReviewStream(store: store, reader: .changes, files: files, split: store.readsSplit(width: width))
        } else if let error = store.reviewError {
            ContentUnavailableView("Comparison unavailable", systemImage: "doc", description: Text(error))
                .padding(.vertical, 24)
        } else {
            ProgressView().frame(maxWidth: .infinity).padding(24)
        }
    }

    // MARK: Footer

    @ViewBuilder private var footer: some View {
        VStack(alignment: .leading, spacing: 4) {
            if let compared = store.comparisonDescription {
                Text(compared).font(Typography.code(10)).foregroundStyle(palette.textSecondary)
                    .accessibilityIdentifier("changes-comparison")
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
