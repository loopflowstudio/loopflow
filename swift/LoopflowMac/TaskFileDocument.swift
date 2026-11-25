#if os(macOS)
import AppKit
import Combine
import Loopflow
import Observation

extension UserDefaults {
    static let taskFileAutosaveKey = "taskFilesAutosave"

    var savesTaskFilesAutomatically: Bool {
        object(forKey: Self.taskFileAutosaveKey) as? Bool ?? true
    }
}

/// The working document. Its native editor stays with it after the first edit.
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
    private var parsedOutline: (version: Int, outline: FileOutline)?
    var outline: FileOutline? { parsedOutline?.version == editVersion ? parsedOutline?.outline : nil }

    func receive(_ outline: FileOutline, version: Int) {
        if version == editVersion { parsedOutline = (version, outline) }
    }

    func readOutline() async {
        guard outline == nil else { return }
        let version = editVersion
        let result = await Task.detached(priority: .userInitiated) { [text, path] in
            FileOutline(tree: Grammar.named(path)?.parse(text), text: text, path: path)
        }.value
        if !Task.isCancelled { receive(result, version: version) }
    }
    @ObservationIgnored private(set) var retainedEditor: TaskFileTextView?
    @ObservationIgnored private lazy var undo = UndoManager()
    @ObservationIgnored lazy var scroll: NSScrollView = {
        let scroll = NSScrollView()
        scroll.hasVerticalScroller = true
        scroll.documentView = editor
        return scroll
    }()

    private var composing: Bool { retainedEditor?.hasMarkedText() == true }
    private(set) var canEdit = false {
        didSet { retainedEditor?.isEditable = canEdit }
    }
    private var incomingGeneration = 0
    private var deferredSnapshot: TaskFileSnapshot?
    private var applyingIncoming = false
    private(set) var saveFailed = false
    private(set) var readOnlyReason: String?
    var onEdit: (() -> Void)?
    /// The theme the editor shows, once it has been on screen.
    @ObservationIgnored private var theme: ThemeDefinition?
    @ObservationIgnored private var coloringScheduled = false
    /// The last highlights worked out, and the `editVersion` of the text they read.
    @ObservationIgnored private var colored: (version: Int, highlights: [Grammar.Highlight])?
    @ObservationIgnored private var highlighting = false

    var canSave: Bool {
        dirty && !saving && external == nil && deferredSnapshot == nil
            && canEdit && !composing
    }

    func reconcile(_ next: TaskFileSnapshot) async {
        incomingGeneration += 1
        let generation = incomingGeneration
        updateAccess(next)
        // Content reads skip history; keep what the last Save disclosed.
        if !next.recoveries.isEmpty { recoveries = next.recoveries }
        guard !saving, !composing else {
            deferredSnapshot = next
            return
        }
        deferredSnapshot = nil
        guard let base = snapshot?.content, let incoming = next.content, next.state == .text else {
            loadSnapshot(next)
            onEdit?()
            return
        }
        if next.revision == snapshot?.revision {
            snapshot = next
            external = nil
            onEdit?()
            return
        }
        if !dirty, retainedEditor == nil {
            loadSnapshot(next)
            onEdit?()
            saveMessage = "Updated from disk"
            return
        }
        let local = text, version = editVersion
        let (merged, edits) = await Task.detached(priority: .userInitiated) {
            let merged = TaskTextEdit.merge(base: base, local: local, disk: incoming)
            return (merged, TaskTextEdit.between(local, merged))
        }.value
        guard !Task.isCancelled, generation == incomingGeneration, snapshot?.content == base else { return }
        guard !saving, !composing else {
            deferredSnapshot = next
            return
        }
        if version != editVersion { await reconcile(next); return }
        snapshot = next
        external = nil
        applyIncoming(merged, edits: edits)
        // Native Undo contains offsets into the old disk version. Rebase its
        // boundary to the surviving local draft; Undo must never restore LLM text.
        if retainedEditor != nil {
            undo.removeAllActions()
            if merged != incoming { registerLocalUndo(incoming) }
        }
        saveMessage = "Updated from disk"
        onEdit?()
    }

    /// The file where the Task runs now, after a move. A draft on another
    /// revision stays whole beside it, never merged: only a save at the
    /// draft's base revision may write it there.
    func arrive(_ next: TaskFileSnapshot) async {
        guard dirty, next.revision != snapshot?.revision else { return await reconcile(next) }
        external = next
    }

    private func applyIncoming(_ value: String, edits: [TaskTextEdit]? = nil) {
        guard value != text else { return }
        guard let editor = retainedEditor else {
            text = value
            editVersion += 1
            return
        }
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
    }

    var editor: TaskFileTextView {
        if let retainedEditor { return retainedEditor }
        let editor = TaskFileTextView(usingTextLayoutManager: true)
        retainedEditor = editor
        editor.isRichText = false
        editor.allowsUndo = true
        editor.usesFindBar = false
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
        editor.string = text
        editor.isEditable = canEdit
        editor.textStorage?.delegate = self
        if dirty, let base = snapshot?.content { registerLocalUndo(base) }
        return editor
    }

    /// The editor's colors, syntax included. Setting the text color clears
    /// every syntax color, so they are painted again after it.
    func applyTheme(_ theme: ThemeDefinition) {
        if self.theme == theme { return }
        self.theme = theme
        editor.applyPalette(theme.palette)
        paintSyntax()
        if colored?.version != editVersion { colorSyntax() }
    }

    /// Parse the buffer off the main thread, one parse at a time. Colors are
    /// painted only for the text they were worked out from; typing meanwhile
    /// keeps the colors already shown and parses again once this one ends.
    private func colorSyntax() {
        guard theme != nil, !highlighting else { return }
        highlighting = true
        let version = editVersion, text = editor.string, path = path
        Task { [weak self] in
            let highlights = await Task.detached(priority: .userInitiated) {
                Grammar.named(path)?.highlights(of: text) ?? []
            }.value
            guard let self else { return }
            self.highlighting = false
            if self.editVersion == version {
                self.colored = (version, highlights)
                self.paintSyntax()
            } else {
                self.colorSyntax()
            }
        }
    }

    private func paintSyntax() {
        guard let theme, let colored, colored.version == editVersion, !composing,
              let storage = editor.textStorage, let font = editor.font else { return }
        let inks = SyntaxInks.of(theme)
        let whole = NSRange(location: 0, length: storage.length)
        storage.beginEditing()
        storage.addAttribute(.foregroundColor, value: NSColor(theme.palette.text), range: whole)
        storage.addAttribute(.font, value: font, range: whole)
        let bold = NSFontManager.shared.convert(font, toHaveTrait: .boldFontMask)
        let italic = NSFontManager.shared.convert(font, toHaveTrait: .italicFontMask)
        for highlight in colored.highlights where NSMaxRange(highlight.range) <= storage.length {
            guard let ink = highlight.ink else { continue }
            switch ink {
            case .emphasis: storage.addAttribute(.font, value: italic, range: highlight.range)
            case .strong: storage.addAttribute(.font, value: bold, range: highlight.range)
            default:
                if ink == .heading { storage.addAttribute(.font, value: bold, range: highlight.range) }
                if let color = inks.color(ink) { storage.addAttribute(.foregroundColor, value: NSColor(color), range: highlight.range) }
            }
        }
        storage.endEditing()
    }

    /// Storage cannot change inside its own editing callback, and `editVersion`
    /// settles after it; color after it.
    private func scheduleSyntaxColors() {
        guard theme != nil, !coloringScheduled else { return }
        coloringScheduled = true
        Task { [weak self] in
            await Task.yield()
            self?.coloringScheduled = false
            self?.colorSyntax()
        }
    }

    func restore(_ draft: WorkFileDraft, task: String) {
        guard !dirty else { return }
        snapshot = TaskFileSnapshot(recoveries: [], issueIdentifier: task, taskId: task, path: draft.path,
            content: draft.baseContent, data: nil, state: .text, revision: draft.baseRevision,
            readOnlyReason: "Recovered draft; checking the current file", sizeBytes: UInt64(draft.baseContent.utf8.count))
        applyIncoming(draft.text)
        canEdit = false
        readOnlyReason = "Recovered draft; checking the current file"
    }

    private func loadSnapshot(_ next: TaskFileSnapshot) {
        guard !dirty, !composing else {
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
        retainedEditor?.string = text
        retainedEditor?.undoManager?.removeAllActions()
        editVersion += 1
    }

    private func updateAccess(_ next: TaskFileSnapshot) {
        readOnlyReason = next.readOnlyReason
        canEdit = next.state == .text && next.readOnlyReason == nil
        onEdit?()
    }

    func save(issue: String, query: RegistryQuery) async {
        guard canSave, let revision = snapshot?.revision else { return }
        saving = true
        saveFailed = false
        let submitted = text
        var refreshAfterSave = false
        do {
            let result = try await query.saveTaskFile(issue: issue, path: path, revision: revision,
                                                      content: submitted)
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
            do { await reconcile(try await query.taskFile(issue: issue, path: path)) }
            catch { saveMessage = "Saved; refresh failed: \(error.localizedDescription)" }
        } else if let pending = deferredSnapshot { await reconcile(pending) }
        onEdit?()
    }

    func undoManager(for view: NSTextView) -> UndoManager? { undo }

    func textStorage(_ storage: NSTextStorage, didProcessEditing mask: NSTextStorageEditActions,
                     range: NSRange, changeInLength: Int) {
        if mask.contains(.editedCharacters) { scheduleSyntaxColors() }
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
            if let pending = self.deferredSnapshot, !self.composing, !self.saving {
                await self.reconcile(pending)
            }
            self.onEdit?()
        }
    }
}

@MainActor
final class TaskFileTextView: NSTextView {
    var compositionEnded: (() -> Void)?
    var focusOnMount = false

    override func viewDidMoveToWindow() {
        super.viewDidMoveToWindow()
        if let window, focusOnMount {
            focusOnMount = false
            window.makeFirstResponder(self)
        }
    }

    override func unmarkText() {
        super.unmarkText()
        compositionEnded?()
    }
}

#endif
