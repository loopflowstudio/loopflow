#if os(macOS)
import Loopflow
import SwiftUI

struct TaskPullRequestView: View {
    @Bindable var store: PullRequestStore
    let url: URL?
    @Environment(\.palette) private var palette
    @State private var showsChecks = false

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 16) {
                HStack {
                    Text("Pull request").font(Typography.strong(14))
                    Spacer()
                    if let url {
                        Button { Task { await refresh(url) } } label: {
                            Image(systemName: "arrow.clockwise")
                        }.buttonStyle(.plain).disabled(store.refreshing)
                            .accessibilityLabel("Refresh pull request")
                        Link(destination: url) { Image(systemName: "arrow.up.right.square") }
                            .accessibilityLabel("Open pull request on GitHub")
                    }
                }
                if let error = store.error {
                    Text(error).font(.caption).foregroundStyle(palette.warning)
                    if store.pullRequest != nil { Text("Showing the last successful reading").font(.caption) }
                }
                if let pr = store.pullRequest {
                    overview(pr)
                    Divider()
                    checks(pr)
                    Divider()
                    reviewers(pr)
                    Divider()
                    VStack(alignment: .leading, spacing: 8) {
                        Text("Description").font(Typography.strong(13))
                        if pr.body.isEmpty { Text("No description").foregroundStyle(palette.textSecondary) }
                        else { MarkdownBlocks(source: pr.body).equatable() }
                    }
                    Divider()
                    fileList(pr)
                    if let updated = store.updatedAt {
                        Text("Updated \(updated, style: .relative) ago")
                            .font(.caption2).foregroundStyle(palette.textSecondary)
                    }
                } else if store.refreshing {
                    ProgressView("Reading GitHub…").frame(maxWidth: .infinity)
                } else if url == nil {
                    Text("No pull request published yet.").foregroundStyle(palette.textSecondary)
                }
            }
            .font(Typography.body(12))
            .padding(14)
            .frame(maxWidth: .infinity, alignment: .leading)
            .textSelection(.enabled)
        }
        .background(palette.background)
        .accessibilityIdentifier("browser-pull-request")
        .task(id: url) { await store.refresh(url) }
        .task(id: "\(store.showsFiles)|\(store.updatedAt?.timeIntervalSince1970 ?? 0)") {
            if store.showsFiles { await store.loadFiles() }
        }
    }

    private func refresh(_ url: URL) async {
        await store.refresh(url)
    }

    private func overview(_ pr: GitHubPullRequest) -> some View {
        VStack(alignment: .leading, spacing: 8) {
            HStack {
                Text(pr.statusLabel).font(Typography.strong(11))
                    .padding(.horizontal, 8).padding(.vertical, 4)
                    .background(palette.surfaceMuted, in: Capsule())
                Text("#\(pr.number)").foregroundStyle(palette.textSecondary)
                Spacer()
            }
            Text(pr.title).font(Typography.strong(18)).fixedSize(horizontal: false, vertical: true)
            Text("\(pr.headRefName) → \(pr.baseRefName)")
                .font(Typography.code(10)).foregroundStyle(palette.textSecondary)
                .lineLimit(2).truncationMode(.middle)
            HStack(spacing: 10) {
                Text("\(pr.changedFiles) files")
                Text("+\(pr.additions)").foregroundStyle(.green)
                Text("−\(pr.deletions)").foregroundStyle(.red)
            }.font(Typography.code(12))
        }
    }

    private func checks(_ pr: GitHubPullRequest) -> some View {
        VStack(alignment: .leading, spacing: 8) {
            Text("Checks").font(Typography.strong(13))
            if let checks = pr.statusCheckRollup {
                if checks.isEmpty { Text("No checks reported").foregroundStyle(palette.textSecondary) }
                if !checks.isEmpty {
                    DisclosureGroup(isExpanded: $showsChecks) {
                        ForEach(Array(checks.enumerated()), id: \.offset) { _, check in
                            HStack(alignment: .top, spacing: 8) {
                                Image(systemName: check.failed ? "xmark.circle" : check.passed ? "checkmark.circle" : "clock")
                                    .foregroundStyle(check.failed ? Color.red : check.passed ? Color.green : palette.textSecondary)
                                VStack(alignment: .leading, spacing: 2) {
                                    if let link = check.link { Link(check.label, destination: link) }
                                    else { Text(check.label) }
                                    Text(check.result.replacingOccurrences(of: "_", with: " ").capitalized)
                                        .font(.caption2).foregroundStyle(palette.textSecondary)
                                }
                                Spacer(minLength: 0)
                            }
                        }
                    } label: {
                        Text(pr.checksSummary)
                            .font(.caption)
                    }
                }
            } else { Text("Check status unavailable").foregroundStyle(palette.textSecondary) }
        }
    }

    private func reviewers(_ pr: GitHubPullRequest) -> some View {
        VStack(alignment: .leading, spacing: 8) {
            Text("Review").font(Typography.strong(13))
            Text(pr.reviewLabel)
            if pr.reviewRequests.isEmpty {
                Text("No pending review requests").foregroundStyle(palette.textSecondary)
            } else {
                Text("Waiting on reviewers").font(Typography.strong(11))
                ForEach(Array(pr.reviewRequests.enumerated()), id: \.offset) { _, reviewer in
                    Label(reviewer.label, systemImage: "person.crop.circle")
                }
            }
        }
    }

    private func coloredPatch(_ patch: String) -> AttributedString {
        var result = AttributedString()
        for line in patch.split(separator: "\n", omittingEmptySubsequences: false) {
            var text = AttributedString(String(line) + "\n")
            if line.hasPrefix("+") { text.foregroundColor = .green }
            else if line.hasPrefix("-") { text.foregroundColor = .red }
            else if line.hasPrefix("@@") { text.foregroundColor = palette.textSecondary }
            result.append(text)
        }
        return result
    }

    private func fileList(_ pr: GitHubPullRequest) -> some View {
        DisclosureGroup(isExpanded: $store.showsFiles) {
            VStack(alignment: .leading, spacing: 10) {
                if let error = store.filesError {
                    Text(error).font(.caption).foregroundStyle(palette.warning)
                    Button("Retry diffs") { Task { await refresh(pr.url) } }
                }
                if store.loadingFiles { ProgressView().controlSize(.small) }
                if let files = store.files {
                    ForEach(files) { file in
                        DisclosureGroup(isExpanded: Binding(
                            get: { store.expandedFiles.contains(file.id) },
                            set: { if $0 { store.expandedFiles.insert(file.id) } else { store.expandedFiles.remove(file.id) } }
                        )) {
                            VStack(alignment: .leading, spacing: 6) {
                                if let previous = file.previousFilename {
                                    Text("Renamed from \(previous)").font(.caption)
                                }
                                if let patch = file.patch, store.expandedFiles.contains(file.id) {
                                    ScrollView(.horizontal) {
                                        Text(coloredPatch(patch)).font(Typography.code(10)).fixedSize(horizontal: true, vertical: false)
                                            .padding(8)
                                    }.background(palette.surfaceMuted, in: RoundedRectangle(cornerRadius: 6))
                                }
                                if file.incompletePatch {
                                    Text("GitHub omitted some or all of this patch (large or binary file).")
                                        .font(.caption).foregroundStyle(palette.textSecondary)
                                }
                                if let link = file.blobURL { Link("Open file on GitHub ↗", destination: link).font(.caption) }
                            }
                        } label: {
                            VStack(alignment: .leading, spacing: 3) {
                                Text(file.filename).font(Typography.code(11)).lineLimit(2).truncationMode(.middle)
                                HStack(spacing: 6) {
                                    Text(file.status.capitalized).foregroundStyle(palette.textSecondary)
                                    Text("+\(file.additions)").foregroundStyle(.green)
                                    Text("−\(file.deletions)").foregroundStyle(.red)
                                }.font(.caption2)
                            }
                        }
                    }
                    if files.count < pr.changedFiles {
                        Text("Showing \(files.count) of \(pr.changedFiles) files returned by GitHub.")
                            .font(.caption).foregroundStyle(palette.textSecondary)
                    }
                }
            }.padding(.top, 8)
        } label: { Text("Files changed · \(pr.changedFiles)").font(Typography.strong(13)) }
    }
}
#endif
