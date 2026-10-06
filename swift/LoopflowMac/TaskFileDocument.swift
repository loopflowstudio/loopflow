#if os(macOS)
import AppKit
import Combine
import Loopflow
import Observation

/// One native buffer per Task/path. Its view, undo stack and selection outlive navigation.
@MainActor
@Observable
final class TaskFileDocument: NSObject, NSTextViewDelegate, @preconcurrency NSTextStorageDelegate {
    let path: String
    private(set) var snapshot: TaskFileSnapshot?
    private(set) var external: TaskFileSnapshot?
    private(set) var recoveries: [TaskFileRecovery] = []
    private(set) var saveMessage: String?
    private(set) var saving = false
    private(set) var text = ""
    private(set) var editVersion = 0
    let editor = TaskFileTextView(usingTextLayoutManager: true)
    let scroll = NSScrollView()
    private let undo = UndoManager()
    private var incomingGeneration = 0
    private var deferredSnapshot: TaskFileSnapshot?
    private var applyingIncoming = false
    private(set) var saveFailed = false
    private(set) var readOnlyReason: String?
    var onEdit: (() -> Void)?

    var canSave: Bool {
        dirty && !saving && external == nil && deferredSnapshot == nil
            && editor.isEditable && !editor.hasMarkedText()
    }

    func reconcile(_ next: TaskFileSnapshot) async {
        incomingGeneration += 1
        let generation = incomingGeneration
        updateAccess(next)
        // Content reads skip history; keep what the last Save disclosed.
        if !next.recoveries.isEmpty { recoveries = next.recoveries }
        guard !saving, !editor.hasMarkedText() else {
            deferredSnapshot = next
            return
        }
        deferredSnapshot = nil
        guard let base = snapshot?.content, let incoming = next.content, next.state == .text else {
            loadSnapshot(next)
            return
        }
        if next.revision == snapshot?.revision {
            snapshot = next
            external = nil
            onEdit?()
            return
        }
        let local = text, version = editVersion
        let (merged, edits) = await Task.detached(priority: .userInitiated) {
            let merged = TaskTextEdit.merge(base: base, local: local, disk: incoming)
            return (merged, TaskTextEdit.between(local, merged))
        }.value
        guard generation == incomingGeneration, snapshot?.content == base else { return }
        guard !saving, !editor.hasMarkedText() else {
            deferredSnapshot = next
            return
        }
        if version != editVersion { await reconcile(next); return }
        snapshot = next
        external = nil
        applyIncoming(merged, edits: edits)
        // Native Undo contains offsets into the old disk version. Rebase its
        // boundary to the surviving local draft; Undo must never restore LLM text.
        undo.removeAllActions()
        if merged != incoming { registerLocalUndo(incoming) }
        saveMessage = "Updated from disk"
        onEdit?()
    }

    private func applyIncoming(_ value: String, edits: [TaskTextEdit]? = nil) {
        guard value != text else { return }
        let edits = edits ?? TaskTextEdit.between(text, value)
        var selection = editor.selectedRange()
        func moved(_ position: Int) -> Int {
            var result = position
            for edit in edits {
                if position >= NSMaxRange(edit.range) {
                    result += edit.replacement.utf16.count - edit.range.length
                } else if position > edit.range.location {
                    result += edit.range.location - position
                }
            }
            return max(0, min(value.utf16.count, result))
        }
        let end = moved(NSMaxRange(selection))
        selection.location = moved(selection.location)
        selection.length = max(0, end - selection.location)
        editor.breakUndoCoalescing()
        applyingIncoming = true
        for edit in edits.reversed() {
            editor.textStorage?.replaceCharacters(in: edit.range, with: edit.replacement)
        }
        applyingIncoming = false
        text = value
        editor.setSelectedRange(selection)
    }

    private func registerLocalUndo(_ value: String) {
        undo.registerUndo(withTarget: editor) { [weak self] _ in
            MainActor.assumeIsolated {
                guard let self else { return }
                self.registerLocalUndo(self.text)
                self.applyIncoming(value)
                self.onEdit?()
            }
        }
        undo.setActionName("Local file edits")
    }

    var dirty: Bool { text != snapshot?.content && snapshot?.content != nil }

    init(path: String) {
        self.path = path
        super.init()
        editor.isRichText = false
        editor.allowsUndo = true
        editor.usesFindBar = true
        editor.isAutomaticQuoteSubstitutionEnabled = false
        editor.isAutomaticDashSubstitutionEnabled = false
        editor.isAutomaticTextReplacementEnabled = false
        editor.isAutomaticSpellingCorrectionEnabled = false
        editor.font = .monospacedSystemFont(ofSize: 12, weight: .regular)
        editor.textContainerInset = NSSize(width: 12, height: 12)
        editor.isVerticallyResizable = true
        editor.isHorizontallyResizable = false
        editor.autoresizingMask = [.width]
        editor.textContainer?.widthTracksTextView = true
        editor.delegate = self
        editor.compositionEnded = { [weak self] in
            Task { @MainActor [weak self] in
                guard let self else { return }
                if let pending = self.deferredSnapshot { await self.reconcile(pending) }
                self.onEdit?()
            }
        }
        editor.textStorage?.delegate = self
        scroll.hasVerticalScroller = true
        scroll.documentView = editor
    }

    private func loadSnapshot(_ next: TaskFileSnapshot) {
        guard !dirty, !editor.hasMarkedText() else {
            external = next.revision != snapshot?.revision || next.state != snapshot?.state ? next : nil
            return
        }
        external = nil
        if next.content == snapshot?.content, next.state == snapshot?.state, next.revision == snapshot?.revision {
            snapshot = next
            return
        }
        snapshot = next
        text = next.content ?? ""
        editor.string = text
        editor.undoManager?.removeAllActions()
        editVersion += 1
    }

    private func updateAccess(_ next: TaskFileSnapshot) {
        readOnlyReason = next.readOnlyReason
        editor.isEditable = next.state == .text && next.readOnlyReason == nil
        onEdit?()
    }

    func save(issue: String, cwd: String, query: RegistryQuery) async {
        guard canSave, let revision = snapshot?.revision else { return }
        saving = true
        saveFailed = false
        let submitted = text
        var refreshAfterSave = false
        do {
            let result = try await query.saveTaskFile(issue: issue, path: path, revision: revision,
                                                      content: submitted, cwd: cwd)
            saveMessage = result.message
            updateAccess(result.file)
            recoveries = result.file.recoveries
            if result.published, result.file.content == submitted {
                // Advance the disk baseline without replacing newer typing, selection or Undo.
                snapshot = result.file
                external = nil
                // A concurrent read may precede or follow publication. Read its
                // current name again instead of applying or dropping uncertain bytes.
                refreshAfterSave = deferredSnapshot != nil
                deferredSnapshot = nil
                if dirty { saveMessage = result.message + " Newer edits remain unsaved." }
            } else {
                // Resolve after the write has settled; never merge against its old baseline.
                deferredSnapshot = result.file
            }
        } catch {
            saveFailed = true
            saveMessage = "Save could not be confirmed: \(error.localizedDescription). Draft retained. Refresh to inspect recovery."
        }
        saving = false
        if refreshAfterSave {
            do { await reconcile(try await query.taskFile(issue: issue, path: path, cwd: cwd)) }
            catch { saveMessage = "Saved; refresh failed: \(error.localizedDescription)" }
        } else if let pending = deferredSnapshot { await reconcile(pending) }
        onEdit?()
    }

    func undoManager(for view: NSTextView) -> UndoManager? { undo }

    func textStorage(_ storage: NSTextStorage, didProcessEditing mask: NSTextStorageEditActions,
                     range: NSRange, changeInLength: Int) {
        // Undo can change storage without a textDidChange delegate callback.
        guard mask.contains(.editedCharacters), text != storage.string else { return }
        text = storage.string
        saveMessage = nil
        saveFailed = false
        editVersion += 1
        guard !applyingIncoming else { return }
        // Marked-text status settles after NSTextStorage's callback.
        Task { [weak self] in
            await Task.yield()
            guard let self else { return }
            if let pending = self.deferredSnapshot, !self.editor.hasMarkedText(), !self.saving {
                await self.reconcile(pending)
            }
            self.onEdit?()
        }
    }
}

@MainActor
final class TaskFileTextView: NSTextView {
    var compositionEnded: (() -> Void)?

    override func unmarkText() {
        super.unmarkText()
        compositionEnded?()
    }
}

struct TaskPatchLine: Sendable {
    let range: NSRange
    let added: Bool

    /// Added and removed lines inside hunks. File headers such as `+++ b/path` stay plain.
    static func changes(in patch: String) -> [TaskPatchLine] {
        var lines: [TaskPatchLine] = []
        var inHunk = false
        let source = patch as NSString
        source.enumerateSubstrings(in: NSRange(location: 0, length: source.length), options: .byLines) { line, range, _, _ in
            guard let line else { return }
            if line.hasPrefix("@@") {
                inHunk = true
            } else if line.hasPrefix("diff ") {
                inHunk = false
            } else if inHunk, line.hasPrefix("+") || line.hasPrefix("-") {
                lines.append(TaskPatchLine(range: range, added: line.hasPrefix("+")))
            }
        }
        return lines
    }
}

@MainActor
@Observable
final class TaskFilesStore {
    enum Mode: String, CaseIterable {
        case diff = "Diff"
        case file = "File"
    }

    let issue: String
    let cwd: String
    let query: RegistryQuery
    var selection: String?
    var mode = Mode.file
    var base = "parent" {
        didSet { if base != oldValue { changes = nil; diff = nil } }
    }
    private(set) var changes: TaskChangesSnapshot?
    private(set) var directories: [String: TaskDirectory] = [:]
    private(set) var directoryErrors: [String: String] = [:]
    private(set) var changesError: String?
    var expandedDirectories: Set<String> = []
    var showIgnored = false
    var showsChanges = false
    var needsComparison: Bool { showsChanges || mode == .diff }
    private var directoryGenerations: [String: Int] = [:]
    private(set) var documents: [String: TaskFileDocument] = [:]
    private(set) var diff: TaskDiffSnapshot?
    private(set) var patchLines: [TaskPatchLine] = []
    private(set) var error: String?
    private var refreshGeneration = 0
    private var fileGeneration = 0
    private var diffGeneration = 0
    private var savedPreference = true
    private var preferenceChanges: AnyCancellable?
    private var observation: TaskFileObservation?
    private var invalidation: Task<Void, Never>?
    private var changedPaths: Set<String> = []
    private var autosaves: [String: Task<Void, Never>] = [:]
    var autosave = true {
        didSet { for document in documents.values { scheduleSave(document) } }
    }

    func observeFiles() {
        guard observation == nil else { return }
        do {
            observation = try TaskFileObservation(path: cwd) { [weak self] paths in
                self?.invalidate(paths)
            }
        } catch { self.error = "Live file observation unavailable: \(error.localizedDescription)" }
    }

    private func invalidate(_ paths: [String]) {
        // A commit, checkout or staging moved what the comparison reads.
        var comparisonChanged = false
        for path in paths {
            if path == ".git" || path.hasPrefix(".git/") {
                // Git and lf write other files here on every read; these move with the comparison.
                let name = (path as NSString).lastPathComponent
                if ["HEAD", "ORIG_HEAD", "index"].contains(name) || path.contains("/refs/") {
                    comparisonChanged = true
                }
                continue
            }
            changedPaths.insert(path)
        }
        guard !changedPaths.isEmpty || comparisonChanged else { return }
        invalidation?.cancel()
        invalidation = Task { [weak self] in
            do { try await Task.sleep(for: .milliseconds(150)) } catch { return }
            guard let self else { return }
            let changed = self.changedPaths
            self.changedPaths.removeAll()
            defer { if Task.isCancelled { self.changedPaths.formUnion(changed) } }
            // Re-read retained documents even when another file or terminal is visible.
            for (path, document) in self.documents where changed.contains(where: {
                $0.isEmpty || $0 == path || path.hasPrefix($0 + "/")
            }) {
                do {
                    let revision = document.snapshot?.revision
                    let next = try await self.query.taskFile(issue: self.issue, path: path, cwd: self.cwd)
                    guard !Task.isCancelled else { return }
                    guard revision == document.snapshot?.revision else {
                        self.invalidate([path])
                        return
                    }
                    await document.reconcile(next)
                } catch { self.error = error.localizedDescription }
            }
            guard !Task.isCancelled else { return }
            for directory in Array(self.directories.keys) where changed.contains(where: {
                let parent = ($0 as NSString).deletingLastPathComponent
                if ($0 as NSString).lastPathComponent == ".gitignore" {
                    return parent.isEmpty || directory == parent || directory.hasPrefix(parent + "/")
                }
                return $0.isEmpty || $0 == directory || directory.hasPrefix($0 + "/")
                    || parent == directory
            }) {
                await self.loadDirectory(directory)
            }
            if self.needsComparison { await self.refreshChanges() }
            if !Task.isCancelled, self.mode == .diff { await self.loadDiff() }
        }
    }

    private func scheduleSave(_ document: TaskFileDocument) {
        autosaves[document.path]?.cancel()
        guard autosave, document.canSave, !document.saveFailed else { return }
        autosaves[document.path] = Task { [weak self, weak document] in
            // One recovery version per settled edit burst, never per keystroke.
            do { try await Task.sleep(for: .seconds(2)) } catch { return }
            guard let self, let document, self.autosave, document.canSave else { return }
            self.autosaves[document.path] = nil
            await document.save(issue: self.issue, cwd: self.cwd, query: self.query)
        }
    }

    init(issue: String, cwd: String, query: RegistryQuery = RegistryQueryLocal.shared) {
        self.issue = issue
        self.cwd = cwd
        self.query = query
        autosave = UserDefaults.standard.object(forKey: "taskFilesAutosave") as? Bool ?? true
        savedPreference = autosave
        preferenceChanges = NotificationCenter.default.publisher(for: UserDefaults.didChangeNotification)
            .sink { [weak self] _ in
                Task { @MainActor [weak self] in
                    guard let self else { return }
                    let enabled = UserDefaults.standard.object(forKey: "taskFilesAutosave") as? Bool ?? true
                    if self.savedPreference != enabled {
                        self.savedPreference = enabled
                        self.autosave = enabled
                    }
                }
            }
    }

    var selectedDocument: TaskFileDocument? { selection.flatMap { documents[$0] } }

    func document(_ path: String) -> TaskFileDocument {
        if let document = documents[path] { return document }
        let document = TaskFileDocument(path: path)
        document.onEdit = { [weak self, weak document] in
            guard let self, let document else { return }
            self.scheduleSave(document)
        }
        documents[path] = document
        return document
    }

    /// Refresh navigation only; selection and filesystem events own document reads.
    func refresh() async {
        await loadDirectory("")
        if needsComparison { await refreshChanges() }
    }

    func loadDirectory(_ path: String, more: Bool = false) async {
        let previous = directories[path]
        guard !more || previous?.nextCursor != nil else { return }
        let generation = (directoryGenerations[path] ?? 0) + 1
        directoryGenerations[path] = generation
        let ignored = showIgnored
        do {
            let page = try await query.taskFiles(issue: issue, directory: path,
                cursor: more ? previous?.nextCursor : nil, showIgnored: ignored, cwd: cwd)
            guard !Task.isCancelled, generation == directoryGenerations[path], ignored == showIgnored else { return }
            directories[path] = TaskDirectory(path: page.path,
                entries: (more ? previous?.entries ?? [] : []) + page.entries, nextCursor: page.nextCursor)
            directoryErrors[path] = nil
            if selection == nil { selection = page.entries.first(where: { $0.kind == .file })?.path }
        } catch {
            guard !Task.isCancelled, generation == directoryGenerations[path], ignored == showIgnored else { return }
            directoryErrors[path] = error.localizedDescription
        }
    }

    func refreshDirectories() async {
        for path in Set(directories.keys).union([""]) { await loadDirectory(path) }
    }

    func refreshChanges() async {
        refreshGeneration += 1
        let generation = refreshGeneration
        let base = base
        do {
            let changes = try await query.taskChanges(issue: issue, base: base, cwd: cwd)
            guard !Task.isCancelled, generation == refreshGeneration, self.base == base else { return }
            self.changes = changes
            if selection == nil { selection = changes.scratch.first ?? changes.files.first?.path }
            changesError = nil
        } catch {
            guard !Task.isCancelled, generation == refreshGeneration else { return }
            changes = nil
            diff = nil
            changesError = error.localizedDescription
        }
    }

    func loadFile() async {
        fileGeneration += 1
        let generation = fileGeneration
        guard let path = selection else { return }
        observation?.watch(path)
        let revision = documents[path]?.snapshot?.revision
        do {
            let next = try await query.taskFile(issue: issue, path: path, cwd: cwd)
            guard !Task.isCancelled, selection == path, generation == fileGeneration,
                  documents[path]?.snapshot?.revision == revision else { return }
            await document(path).reconcile(next)
            error = nil
        } catch {
            guard !Task.isCancelled, selection == path, generation == fileGeneration else { return }
            self.error = error.localizedDescription
        }
    }

    func loadDiff() async {
        if changes == nil { await refreshChanges() }
        diffGeneration += 1
        let generation = diffGeneration
        // Keep the displayed patch for the same document while its replacement loads.
        if diff?.path != selection || diff?.baseCommit != changes?.baseCommit { diff = nil }
        guard let path = selection, let base = changes?.baseCommit else { return }
        observation?.watch(path)
        let document = selectedDocument
        let version = document?.editVersion
        do {
            // Debounce edits; identity checks also protect transports that finish after cancellation.
            if document?.dirty == true { try await Task.sleep(for: .milliseconds(180)) }
            let next = try await query.taskDiff(issue: issue, path: path, base: base,
                                               draft: document?.dirty == true ? document?.text : nil, cwd: cwd)
            let lines = await Task.detached(priority: .userInitiated) {
                TaskPatchLine.changes(in: next.patch)
            }.value
            guard !Task.isCancelled, generation == diffGeneration, selection == path, changes?.baseCommit == base,
                  selectedDocument?.editVersion == version else { return }
            patchLines = lines
            diff = next
            changesError = nil
        } catch {
            guard !Task.isCancelled, generation == diffGeneration, selection == path, changes?.baseCommit == base else { return }
            changesError = error.localizedDescription
        }
    }
}
#endif
