import Loopflow
import SwiftUI

/// A Task's checkout as the phone browses it, read from wherever the Task
/// runs: each directory opened so far, one page at a time. The checkout's
/// `task.files.watch` readings re-read every open directory when the tree
/// changes or the Task moves to another Machine.
@MainActor
@Observable
final class TaskFilesTree {
    struct Listing: Equatable {
        var entries: [TaskFileEntry]
        var nextCursor: String?
    }

    let issue: String
    /// Each opened directory by path; the empty path is the root.
    private(set) var listings: [String: Loaded<Listing>] = [:]
    /// The Task has no checkout yet, wherever it runs.
    private(set) var notStarted = false
    /// Why the live reading stopped, naming the Machine when it is away; nil while it follows the Task.
    private(set) var unavailable: String?
    /// The Machine and checkout of the last reading; a different one is a move.
    private(set) var machineID: String?
    private(set) var checkout: String?
    @ObservationIgnored private let client: RemoteMobileClient
    @ObservationIgnored private var revision: String?

    init(client: RemoteMobileClient, issue: String) {
        self.client = client
        self.issue = issue
    }

    /// Read `path`'s first page again.
    func load(_ path: String) async {
        do {
            let page = try await client.query.taskFiles(issue: issue, directory: path)
            notStarted = false
            listings[path] = .value(Listing(entries: page.entries, nextCursor: page.nextCursor))
        } catch let refusal as ServerError where refusal.notStarted {
            notStarted = true
            listings[path] = nil
        } catch {
            listings[path] = .failed(machineConnectionMessage(error))
        }
    }

    /// The next page of `path`, after what is already listed.
    func more(_ path: String) async {
        guard case .value(var listing) = listings[path], let cursor = listing.nextCursor else { return }
        do {
            let page = try await client.query.taskFiles(issue: issue, directory: path, cursor: cursor)
            listing.entries += page.entries
            listing.nextCursor = page.nextCursor
            listings[path] = .value(listing)
        } catch {
            listings[path] = .failed(machineConnectionMessage(error))
        }
    }

    /// Re-read every opened directory, the root first.
    func reload() async {
        let paths = Set(listings.keys).union([""]).sorted()
        for path in paths { await load(path) }
    }

    /// Follow the Task's checkout until cancelled. The first reading is the
    /// tree already read; a later revision, Machine or checkout re-reads it.
    /// A reading with no checkout is **Not started**, and the same reading
    /// fills Files in once the Task starts. An outage keeps what was listed
    /// and says which Machine is away.
    func follow() async {
        let watch = client.observeTaskFiles(issue: issue)
        defer { watch.cancel() }
        do {
            for try await frame in watch.updates {
                let reading = try JSONDecoder().decode(SubscriptionFrame<TaskFilesRevision>.self, from: frame.bytes)
                guard let body = reading.body else {
                    unavailable = reading.unavailable ?? "Live file observation unavailable"
                    continue
                }
                let recovered = unavailable != nil
                unavailable = nil
                let changed = machineID != nil
                    && (body.machineID != machineID || body.checkout != checkout || body.revision != revision)
                machineID = body.machineID
                checkout = body.checkout
                revision = body.revision
                if body.notStarted {
                    notStarted = true
                    listings = [:]
                } else if changed || recovered || notStarted {
                    await reload()
                }
            }
        } catch is CancellationError {
            return
        } catch {
            unavailable = machineConnectionMessage(error)
        }
    }
}

/// One directory of the Task's checkout: folders open the next level, files
/// open as they stand, changed or not.
struct TaskFilesTreeView: View {
    let client: RemoteMobileClient
    let task: MobileTask
    let tree: TaskFilesTree
    let path: String
    @Environment(\.theme) private var theme

    var body: some View {
        Group {
            if tree.notStarted {
                StateView("Not started", sentence: "The Task has no checkout yet. Files appear once it starts.")
            } else {
                switch tree.listings[path] {
                case nil: ProgressView()
                case .loading: ProgressView()
                case .failed(let message):
                    StateView("Couldn’t load", sentence: message) {
                        Button("Try again") { Task { await tree.load(path) } }.buttonStyle(LoopflowButtonStyle(.accent))
                    }
                case .value(let listing) where listing.entries.isEmpty:
                    StateView("Empty folder")
                case .value(let listing):
                    list(listing)
                }
            }
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
        .background(theme.palette.background)
        .task { if tree.listings[path] == nil { await tree.load(path) } }
    }

    private func list(_ listing: TaskFilesTree.Listing) -> some View {
        List {
            if let unavailable = tree.unavailable {
                Text(unavailable)
                    .font(Typography.meta)
                    .foregroundStyle(theme.palette.warning)
                    .listRowBackground(theme.palette.background)
            }
            ForEach(listing.entries) { entry in
                NavigationLink {
                    if entry.kind == .directory {
                        TaskFilesTreeView(client: client, task: task, tree: tree, path: entry.path)
                            .navigationTitle((entry.path as NSString).lastPathComponent)
                            .navigationBarTitleDisplayMode(.inline)
                    } else {
                        TaskFileView(client: client, task: task, path: entry.path)
                    }
                } label: {
                    Label((entry.path as NSString).lastPathComponent,
                          systemImage: entry.kind == .directory ? "folder" : "doc")
                        .font(Typography.text)
                        .foregroundStyle(theme.palette.text)
                }
                .listRowBackground(theme.palette.background)
            }
            if listing.nextCursor != nil {
                Button("Show more") { Task { await tree.more(path) } }
                    .listRowBackground(theme.palette.background)
            }
        }
        .listStyle(.plain)
        .refreshable { await tree.load(path) }
    }
}
