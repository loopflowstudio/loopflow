import AppKit
import Loopflow
import Observation

/// Mode plus the Light and Dark theme names. Both names live in memory;
/// the config states only what the mode uses.
struct AppearanceSelection: Equatable {
    var mode: AppearanceMode = .system
    var light = ThemeDefinition.light.name
    var dark = ThemeDefinition.dark.name

    func name(dark: Bool) -> String { dark ? self.dark : light }
    mutating func setName(_ name: String, dark: Bool) { if dark { self.dark = name } else { light = name } }
    func isDark(systemDark: Bool) -> Bool { mode == .dark || (mode == .system && systemDark) }

    /// System writes the pair; Light or Dark writes only that side's name.
    var configValue: String {
        switch mode {
        case .system: return "light:\(light),dark:\(dark)"
        case .light: return light
        case .dark: return dark
        }
    }
}

struct ThemeDeclaration: Decodable {
    let entries: [ConfigurationEntry]
    let value: JSONValue
}

/// One personal theme file being edited. A nil theme stages removal of that file.
private struct ThemeDraft {
    let name: String
    var theme: ThemeDefinition?
    var selection: AppearanceSelection
    var original: JSONValue
}

/// All windows share one theme choice from `theme:` in `~/.lf/config.yaml`, your theme files and
/// a live preview while editing a theme file.
@MainActor @Observable
final class AppearanceStore {
    static let shared = AppearanceStore(paths: .standard, watches: true,
                                        publish: { try GhosttyManager.shared.applyTheme($0, retaining: $1) })
    let paths: ThemePaths
    private(set) var selection = AppearanceSelection()
    private(set) var library = ThemeLibrary()
    private var declaration = JSONValue.null
    private var draft: ThemeDraft?
    private(set) var previewSide: AppearanceMode = .light
    private(set) var effectiveTheme = ThemeDefinition.light
    private(set) var systemDark: Bool
    private(set) var diagnostics: [String] = []
    /// What Choose Theme can fix: an unknown name, an unreadable theme file.
    var notice: String?
    /// The server couldn't read or write `theme:`; choosing again sends the same request.
    private(set) var readFailure: String?
    @ObservationIgnored private let location: ServerLocation
    private(set) var saving = false
    @ObservationIgnored private var reading = UUID()
    @ObservationIgnored private let publish: (ThemeDefinition, String) throws -> Void
    @ObservationIgnored private var observation: NSKeyValueObservation?
    @ObservationIgnored private var watcher: Timer?
    @ObservationIgnored private var fingerprint: [String] = []
    /// Where the server keeps `theme:`, as its last answer said.
    private(set) var configPath: String?

    init(paths: ThemePaths, systemDark: Bool? = nil, watches: Bool = false, location: ServerLocation = .shared,
         publish: @escaping (ThemeDefinition, String) throws -> Void = { _, _ in }) {
        self.location = location
        self.paths = paths
        self.publish = publish
        self.systemDark = systemDark ?? (NSApplication.shared.effectiveAppearance.bestMatch(from: [.darkAqua, .aqua]) == .darkAqua)
        library = ThemeLibrary.load(paths)
        notice = library.problems.isEmpty ? nil : library.problems.joined(separator: "\n")
        effectiveTheme = resolved(selection, in: library)
        previewSide = self.systemDark ? .dark : .light
        do { try publish(effectiveTheme, effectiveTheme.id) } catch { notice = error.localizedDescription }
        if systemDark == nil {
            observation = NSApplication.shared.observe(\.effectiveAppearance) { [weak self] _, _ in
                Task { @MainActor [weak self] in
                    self?.setSystemDark(NSApplication.shared.effectiveAppearance.bestMatch(from: [.darkAqua, .aqua]) == .darkAqua)
                }
            }
        }
        if watches {
            Task { await reload() }
            watcher = Timer.scheduledTimer(withTimeInterval: 1, repeats: true) { [weak self] _ in
                Task { @MainActor [weak self] in await self?.reloadIfChanged() }
            }
        }
    }

    // MARK: Reading

    var current: AppearanceSelection { draft?.selection ?? selection }
    private var currentLibrary: ThemeLibrary {
        guard let draft else { return library }
        var result = library
        result.yours[draft.name] = draft.theme
        return result
    }
    var removingTheme: String? { draft.flatMap { $0.theme == nil ? $0.name : nil } }
    var editing: Bool { draft != nil }
    var selectedName: String { current.name(dark: previewSide == .dark) }
    var selectedTheme: ThemeDefinition { theme(selectedName, dark: previewSide == .dark, in: currentLibrary) }
    var selectedSource: ThemeEntry.Source? { currentLibrary.source(of: selectedName) }
    func entries(for side: AppearanceMode) -> (builtins: [ThemeEntry], yours: [ThemeEntry]) {
        currentLibrary.entries(dark: side == .dark)
    }
    var canDelete: Bool { selectedSource == .yours }
    var deleteRevertsToBuiltin: Bool { canDelete && currentLibrary.isBuiltin(selectedName) }
    var draftErrors: [String] {
        guard let theme = draft?.theme else { return [] }
        return theme.readabilityErrors.map { "\(theme.name): \($0)" }
    }
    var canApply: Bool { editing && !saving && draftErrors.isEmpty }
    func themeName(for side: AppearanceMode) -> String { current.name(dark: side == .dark) }

    private func theme(_ name: String, dark: Bool, in library: ThemeLibrary) -> ThemeDefinition {
        library.resolve(name) ?? (dark ? .dark : .light)
    }
    private func resolved(_ choice: AppearanceSelection, in library: ThemeLibrary) -> ThemeDefinition {
        let dark = choice.isDark(systemDark: systemDark)
        return theme(choice.name(dark: dark), dark: dark, in: library)
    }

    /// Read your files and `theme:` again. An unknown name keeps the last good choice.
    func reload() async {
        let generation = UUID()
        reading = generation
        let observedFiles = currentFingerprint()
        library = ThemeLibrary.load(paths)
        var problems = library.problems
        let answer: ConfigurationFileValue
        do {
            answer = try JSONDecoder().decode(ConfigurationFileValue.self, from:
                await ServerClient.configuration(.configTheme(value: nil), location: location))
        }
        catch { if reading == generation { readFailure = error.localizedDescription }; return }
        guard reading == generation else { return }
        readFailure = nil
        configPath = answer.path
        let changed = declaration != answer.value
        declaration = answer.value
        fingerprint = observedFiles
        switch selection(from: answer.text) {
        case let .success(choice): selection = choice
        case let .failure(error): problems.insert(error.message, at: 0)
        }
        notice = problems.isEmpty ? nil : problems.joined(separator: "\n")
        if draft == nil {
            if changed { previewSide = selection.isDark(systemDark: systemDark) ? .dark : .light }
            update(resolved(selection, in: library), keepingNotice: true)
        }
    }

    private func selection(from raw: String?) -> Result<AppearanceSelection, ThemeError> {
        guard let raw, let value = ThemeChoice.parse(raw) else { return .success(AppearanceSelection()) }
        let config = configPath ?? "~/.lf/config.yaml"
        func unknown(_ name: String) -> ThemeError {
            ThemeError("No theme named “\(name)”: keeping \(effectiveTheme.name). Fix theme: in \(config) with a built-in name or a file in \(paths.themes.path).")
        }
        var choice = selection
        switch value {
        case let .single(name):
            guard let theme = library.resolve(name) else { return .failure(unknown(name)) }
            choice.mode = theme.isDark ? .dark : .light
            choice.setName(name, dark: theme.isDark)
        case let .pair(light, dark):
            for name in [light, dark].compactMap({ $0 }) where library.resolve(name) == nil {
                return .failure(unknown(name))
            }
            choice.mode = .system
            choice.light = light ?? ThemeDefinition.light.name
            choice.dark = dark ?? ThemeDefinition.dark.name
        }
        return .success(choice)
    }

    private func currentFingerprint() -> [String] {
        let config = configPath.map { [URL(fileURLWithPath: $0)] } ?? []
        let files = config + [paths.themes, paths.ghostty].compactMap { $0 }.flatMap { (try? ThemeFile.directory($0)) ?? [] }
        return files.map { url in
            let values = try? url.resourceValues(forKeys: [.contentModificationDateKey, .fileSizeKey])
            return "\(url.path)|\(values?.contentModificationDate?.timeIntervalSince1970 ?? 0)|\(values?.fileSize ?? -1)"
        }
    }

    /// A failed read retries every tick: until the server answers, its config path is unknown, so
    /// there is no file to watch, and updating the older build should clear the banner on its own.
    private func reloadIfChanged() async {
        guard !saving else { return }
        let latest = currentFingerprint()
        guard latest != fingerprint || readFailure != nil else { return }
        await reload()
    }

    // MARK: Editor

    func setSystemDark(_ dark: Bool) {
        guard systemDark != dark else { return }
        systemDark = dark
        if draft == nil {
            previewSide = selection.isDark(systemDark: dark) ? .dark : .light
            update(resolved(selection, in: library))
        }
    }

    func customize() {
        guard !editing else { return }
        diagnostics = []
        let theme = selectedTheme
        draft = ThemeDraft(name: theme.name, theme: theme, selection: selection, original: declaration)
        update(theme)
    }

    func editTheme(_ edit: (inout PaletteDefinition, inout TerminalPalette) -> Void) {
        if !editing { customize() }
        guard let theme = draft?.theme else { return }
        var palette = theme.palette.definition
        var terminal = theme.terminal
        edit(&palette, &terminal)
        let edited = ThemeDefinition(name: theme.name, palette: LoopflowPalette(palette), terminal: terminal)
        draft?.theme = edited
        update(edited)
    }

    /// Removing a personal override reveals its built-in. Other names return to the default for that side.
    func deleteTheme() {
        guard canDelete else { return }
        customize()
        let builtin = library.isBuiltin(selectedName)
        draft?.theme = nil
        if !builtin {
            let dark = previewSide == .dark
            draft?.selection.setName((dark ? ThemeDefinition.dark : .light).name, dark: dark)
        }
        update(selectedTheme)
    }

    func importTheme(_ url: URL) {
        do {
            let file = try ThemeFile.read(url)
            let theme = file.theme
            var choice = selection
            previewSide = theme.isDark ? .dark : .light
            if choice.mode != .system { choice.mode = previewSide }
            choice.setName(theme.name, dark: theme.isDark)
            draft = ThemeDraft(name: theme.name, theme: theme, selection: choice, original: declaration)
            diagnostics = file.diagnostics
            update(theme)
        } catch { notice = error.localizedDescription }
    }

    // MARK: Writing

    func applyMode(_ mode: AppearanceMode, notices: NoticeCenter? = nil, manager: UndoManager? = nil) async {
        var choice = selection
        choice.mode = mode
        await choose(choice, side: choice.isDark(systemDark: systemDark) ? .dark : .light, notices: notices, manager: manager)
    }

    func selectTheme(_ name: String, notices: NoticeCenter, manager: UndoManager?) async {
        guard let theme = library.resolve(name) else { notice = "No theme named “\(name)”."; return }
        guard theme.readabilityErrors.isEmpty else { notice = theme.readabilityErrors.joined(separator: "\n"); return }
        let side: AppearanceMode = theme.isDark ? .dark : .light
        var choice = selection
        if choice.mode != .system { choice.mode = side }
        choice.setName(name, dark: theme.isDark)
        await choose(choice, side: side, notices: notices, manager: manager)
    }

    private func choose(_ choice: AppearanceSelection, side: AppearanceMode, notices: NoticeCenter?, manager: UndoManager?) async {
        guard !saving, !editing else { return }
        saving = true
        reading = UUID()
        defer { saving = false }
        do {
            let receipt = try await save(choice, original: declaration)
            selection = choice
            previewSide = side
            readFailure = nil
            update(resolved(choice, in: library))
            guard receipt.before != receipt.after else { return }
            let restore: @MainActor (AppearanceStore) -> Void = { [weak notices] store in
                Task {
                    do {
                        _ = try await store.edit(.restore(receipt.before), expected: receipt.after)
                        notices?.clear(key: "theme")
                    } catch {
                        let failure = error.localizedDescription
                        await store.reload()
                        store.readFailure = failure
                        notices?.post(Notice(failure, key: "theme"))
                        return
                    }
                    await store.reload()
                }
            }
            let name = "Change Theme"
            manager?.registerUndo(withTarget: self, handler: restore)
            manager?.setActionName(name)
            notices?.post(Notice("Theme saved", key: "theme", action: .init("Undo") { [weak self, weak manager] in
                if let manager, manager.undoActionName == name { manager.undo() }
                else if let self { restore(self) }
            }))
        } catch {
            await reload()
            readFailure = error.localizedDescription
        }
    }

    private func edit(_ change: ConfigurationChange, expected: [ConfigurationEntry]) async throws -> ConfigurationEditReceipt {
        let data = try await ServerClient.configuration(.edit(destination: .object(["kind": .string("personal_global")]),
            key: "theme", expected: expected, change: change), location: location)
        return try JSONDecoder().decode(ConfigurationEditReceipt.self, from: data)
    }

    private func save(_ choice: AppearanceSelection, original: JSONValue) async throws -> ConfigurationEditReceipt {
        // This file read bypasses the observation cache used by pane subscriptions.
        let data = try await ServerClient.configuration(.configTheme(value: nil), location: location)
        let authored = try JSONDecoder().decode(ThemeDeclaration.self, from: data)
        guard authored.value == original else {
            throw RegistryQueryError("The theme declaration changed. Review the current choice and try again.")
        }
        let receipt = try await edit(.set(JSONValue.string(choice.configValue).settingsYAML), expected: authored.entries)
        declaration = .string(choice.configValue)
        return receipt
    }

    func apply() async {
        guard let draft, canApply else { return }
        saving = true
        reading = UUID()
        defer { saving = false }
        do {
            let url = paths.themes.appendingPathComponent(draft.name)
            if let theme = draft.theme {
                try FileManager.default.createDirectory(at: paths.themes, withIntermediateDirectories: true)
                try Data(ThemeFile.text(theme).utf8).write(to: url, options: .atomic)
            } else if FileManager.default.fileExists(atPath: url.path) {
                try FileManager.default.removeItem(at: url)
            }
            _ = try await save(draft.selection, original: draft.original)
        } catch {
            await reload()
            if readFailure == nil { self.draft?.original = declaration }
            notice = error.localizedDescription
            return
        }
        selection = draft.selection
        self.draft = nil
        readFailure = nil
        diagnostics = []
        library = ThemeLibrary.load(paths)
        notice = library.problems.isEmpty ? nil : library.problems.joined(separator: "\n")
        update(resolved(selection, in: library), keepingNotice: true)
    }

    func cancel() {
        draft = nil
        diagnostics = []
        previewSide = selection.isDark(systemDark: systemDark) ? .dark : .light
        update(resolved(selection, in: library))
    }

    private func update(_ theme: ThemeDefinition, keepingNotice: Bool = false) {
        do {
            guard theme.readabilityErrors.isEmpty else { throw ThemeError(theme.readabilityErrors.joined(separator: "\n")) }
            let theme = AppTestMode.forcesLightAppearance ? ThemeDefinition.light : theme
            if effectiveTheme.id != theme.id || effectiveTheme.terminal != theme.terminal || effectiveTheme.palette.definition != theme.palette.definition {
                try publish(theme, resolved(selection, in: library).id)
            }
            effectiveTheme = theme
            if !keepingNotice { notice = nil }
        } catch { notice = error.localizedDescription }
    }
}
