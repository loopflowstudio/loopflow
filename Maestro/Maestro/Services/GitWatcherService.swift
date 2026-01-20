// Service for watching git directory changes via FSEvents.

import Foundation

enum GitChangeKind: Sendable {
    case worktrees
    case refs
    case index
}

actor GitWatcherService {
    typealias ChangeHandler = @Sendable (GitChangeKind) -> Void

    private var stream: FSEventStreamRef?
    private var onChange: ChangeHandler?
    private var debounceTask: Task<Void, Never>?
    private var pendingChanges: Set<GitChangeKind> = []
    private var watchedRepo: URL?

    private let debounceInterval: Duration = .milliseconds(500)

    func watch(repo: URL, onChange: @escaping ChangeHandler) {
        stop()

        self.watchedRepo = repo
        self.onChange = onChange

        guard let gitDir = findGitDir(at: repo) else { return }

        let paths = [
            gitDir.appendingPathComponent("worktrees").path,
            gitDir.appendingPathComponent("refs").path,
            gitDir.appendingPathComponent("index").path,
        ]
        startStream(paths: paths)
    }

    private func findGitDir(at repo: URL) -> URL? {
        let gitDir = repo.appendingPathComponent(".git")

        var isDir: ObjCBool = false
        if FileManager.default.fileExists(atPath: gitDir.path, isDirectory: &isDir), isDir.boolValue {
            return gitDir
        }

        // This repo might be a worktree itself - resolve the .git file
        return resolveGitDir(at: repo)
    }

    func stop() {
        if let stream = stream {
            FSEventStreamStop(stream)
            FSEventStreamInvalidate(stream)
            FSEventStreamRelease(stream)
        }
        stream = nil
        debounceTask?.cancel()
        debounceTask = nil
        pendingChanges = []
        onChange = nil
        watchedRepo = nil
    }

    private func startStream(paths: [String]) {
        var context = FSEventStreamContext()
        context.info = Unmanaged.passUnretained(self).toOpaque()

        let callback: FSEventStreamCallback = { (
            streamRef,
            clientCallBackInfo,
            numEvents,
            eventPaths,
            eventFlags,
            eventIds
        ) in
            guard let info = clientCallBackInfo else { return }
            let watcher = Unmanaged<GitWatcherService>.fromOpaque(info).takeUnretainedValue()
            let paths = unsafeBitCast(eventPaths, to: NSArray.self) as! [String]

            Task {
                await watcher.handleEvents(paths: paths)
            }
        }

        let pathsToWatch = paths as CFArray

        stream = FSEventStreamCreate(
            nil,
            callback,
            &context,
            pathsToWatch,
            FSEventStreamEventId(kFSEventStreamEventIdSinceNow),
            0.1,  // Latency in seconds (coalesce rapid events)
            UInt32(kFSEventStreamCreateFlagUseCFTypes | kFSEventStreamCreateFlagFileEvents)
        )

        if let stream = stream {
            FSEventStreamSetDispatchQueue(stream, DispatchQueue.main)
            FSEventStreamStart(stream)
        }
    }

    private func handleEvents(paths: [String]) {
        for path in paths {
            let change = classifyChange(path: path)
            if let change = change {
                pendingChanges.insert(change)
            }
        }

        // Debounce: wait for quiet period before firing
        debounceTask?.cancel()
        debounceTask = Task {
            try? await Task.sleep(for: debounceInterval)
            guard !Task.isCancelled else { return }
            await flushPendingChanges()
        }
    }

    private func classifyChange(path: String) -> GitChangeKind? {
        if path.contains("/worktrees/") || path.hasSuffix("/worktrees") {
            return .worktrees
        }
        if path.contains("/refs/") || path.hasSuffix("/refs") {
            return .refs
        }
        if path.hasSuffix("/index") || path.contains("/index.lock") {
            return .index
        }
        return nil
    }

    private func flushPendingChanges() async {
        let changes = pendingChanges
        pendingChanges = []

        guard let handler = onChange else { return }

        for change in changes {
            handler(change)
        }
    }

    private func resolveGitDir(at repo: URL) -> URL? {
        let gitFile = repo.appendingPathComponent(".git")
        guard let contents = try? String(contentsOf: gitFile, encoding: .utf8) else {
            return nil
        }

        // .git file contains "gitdir: /path/to/real/gitdir"
        let prefix = "gitdir: "
        guard contents.hasPrefix(prefix) else { return nil }

        let gitDirPath = contents.dropFirst(prefix.count).trimmingCharacters(in: .whitespacesAndNewlines)

        // Path might be relative
        if gitDirPath.hasPrefix("/") {
            return URL(fileURLWithPath: gitDirPath)
        } else {
            return repo.appendingPathComponent(gitDirPath).standardized
        }
    }
}
