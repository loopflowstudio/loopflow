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

/// The chooser's preview. `changed` names your files to rewrite from `library.yours`, or delete when absent there.
private struct AppearanceDraft {
    var selection: AppearanceSelection
    var library: ThemeLibrary
    var changed: Set<String> = []
}

/// All windows share one theme choice from `theme:` in `~/.lf/config.yaml`, your theme files and
/// one reversible preview. The files are the only store: Apply writes them, Cancel writes nothing.
@MainActor @Observable
final class AppearanceStore {
    static let shared = AppearanceStore(paths: .standard, watches: true,
                                        publish: { try GhosttyManager.shared.applyTheme($0, retaining: $1) })
    let paths: ThemePaths
    private(set) var selection = AppearanceSelection()
    private(set) var library = ThemeLibrary()
    private var draft: AppearanceDraft?
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
    private var currentLibrary: ThemeLibrary { draft?.library ?? library }
    var chooserOpen: Bool { draft != nil }
    var selectedName: String { current.name(dark: previewSide == .dark) }
    var selectedTheme: ThemeDefinition { theme(selectedName, dark: previewSide == .dark, in: currentLibrary) }
    var selectedSource: ThemeEntry.Source? { currentLibrary.source(of: selectedName) }
    func entries(for side: AppearanceMode) -> (builtins: [ThemeEntry], yours: [ThemeEntry]) {
        currentLibrary.entries(dark: side == .dark)
    }
    var canDelete: Bool { selectedSource == .yours }
    var deleteRevertsToBuiltin: Bool { canDelete && currentLibrary.isBuiltin(selectedName) }
    var draftErrors: [String] {
        guard let draft else { return [] }
        return draft.changed.sorted().flatMap { name in
            (draft.library.yours[name]?.readabilityErrors ?? []).map { "\(name): \($0)" }
        }
    }
    var canApply: Bool { !saving && draftErrors.isEmpty }
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
        library = ThemeLibrary.load(paths)
        var problems = library.problems
        let raw: String?
        do { raw = try await readChoice() }
        catch { if reading == generation { readFailure = error.localizedDescription }; return }
        guard reading == generation else { return }
        readFailure = nil
        fingerprint = currentFingerprint()
        switch selection(from: raw) {
        case let .success(choice): selection = choice
        case let .failure(error): problems.insert(error.message, at: 0)
        }
        notice = problems.isEmpty ? nil : problems.joined(separator: "\n")
        if draft == nil {
            previewSide = selection.isDark(systemDark: systemDark) ? .dark : .light
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

    private func reloadIfChanged() async {
        let latest = currentFingerprint()
        guard latest != fingerprint else { return }
        fingerprint = latest
        await reload()
    }

    /// The server's `theme:` text, read or written as the CLI's typed declaration.
    private func readChoice() async throws -> String? {
        try declared(await ServerClient.configuration(.configTheme(value: nil), location: location))
    }

    private func writeChoice(_ value: String) async throws {
        let encoded = String(decoding: try JSONEncoder().encode(value), as: UTF8.self)
        _ = try declared(await ServerClient.configuration(.configTheme(value: encoded), location: location))
    }

    private func declared(_ answer: Data) throws -> String? {
        let declaration = try JSONDecoder().decode(ConfigurationFileValue.self, from: answer)
        configPath = declaration.path
        return declaration.text
    }

    // MARK: Preview

    func setSystemDark(_ dark: Bool) {
        guard systemDark != dark else { return }
        systemDark = dark
        if draft == nil { update(resolved(selection, in: library)) }
        else if current.mode == .system {
            previewSide = dark ? .dark : .light
            update(selectedTheme)
        }
    }
    func beginPreview() {
        guard draft == nil else { return }
        draft = AppearanceDraft(selection: selection, library: library)
        previewSide = selection.isDark(systemDark: systemDark) ? .dark : .light
        diagnostics = []
    }
    func previewMode(_ mode: AppearanceMode) {
        beginPreview()
        draft?.selection.mode = mode
        previewSide = current.isDark(systemDark: systemDark) ? .dark : .light
        update(selectedTheme)
    }
    func previewTheme(_ name: String) {
        guard let theme = currentLibrary.resolve(name) else { notice = "No theme named “\(name)”."; return }
        beginPreview()
        diagnostics = []
        show(theme)
    }

    /// Make the selected theme yours: a built-in or Ghostty theme becomes your file of the same name.
    func customize() {
        beginPreview()
        guard selectedSource != .yours else { return }
        stage(selectedTheme, for: selectedName)
    }

    func editTheme(_ edit: (inout PaletteDefinition, inout TerminalPalette) -> Void) {
        customize()
        var palette = selectedTheme.palette.definition
        var terminal = selectedTheme.terminal
        edit(&palette, &terminal)
        stage(ThemeDefinition(name: selectedName, palette: LoopflowPalette(palette), terminal: terminal), for: selectedName)
    }

    /// Delete your file. One that overrides a built-in reverts to it; otherwise the side returns to Loopflow's.
    func deleteTheme() {
        guard canDelete else { return }
        let name = selectedName
        let builtin = currentLibrary.isBuiltin(name)
        stage(nil, for: name)
        if !builtin {
            let dark = previewSide == .dark
            draft?.selection.setName((dark ? ThemeDefinition.dark : .light).name, dark: dark)
        }
        update(selectedTheme)
    }

    /// Import a theme file onto the side its own colors say.
    func importTheme(_ url: URL) {
        beginPreview()
        do {
            let file = try ThemeFile.read(url)
            stage(file.theme, for: file.theme.name)
            diagnostics = file.diagnostics
            show(file.theme)
        } catch { notice = error.localizedDescription }
    }

    /// Assign `theme` to the side its own colors say; in Light or Dark mode the preview switches to that side.
    private func show(_ theme: ThemeDefinition) {
        let dark = theme.isDark
        if current.mode != .system { draft?.selection.mode = dark ? .dark : .light }
        previewSide = dark ? .dark : .light
        draft?.selection.setName(theme.name, dark: dark)
        update(theme)
    }

    /// Stage your file `name`: a theme to write, or nil to delete.
    private func stage(_ theme: ThemeDefinition?, for name: String) {
        draft?.changed.insert(name)
        draft?.library.yours[name] = theme
        if let theme { update(theme) }
    }

    // MARK: Writing

    /// Menu Mode change: writes the config at once and discards any preview.
    func applyMode(_ mode: AppearanceMode) async {
        guard !saving else { return }
        saving = true
        reading = UUID()
        defer { saving = false }
        var choice = selection
        choice.mode = mode
        do {
            try await writeChoice(choice.configValue)
        } catch { readFailure = error.localizedDescription; return }
        readFailure = nil
        selection = choice
        draft = nil
        update(resolved(selection, in: library))
    }

    func apply() async {
        guard let draft, canApply else { return }
        saving = true
        reading = UUID()
        defer { saving = false }
        do {
            for name in draft.changed.sorted() {
                let url = paths.themes.appendingPathComponent(name)
                if let theme = draft.library.yours[name] {
                    try FileManager.default.createDirectory(at: paths.themes, withIntermediateDirectories: true)
                    try Data(ThemeFile.text(theme).utf8).write(to: url, options: .atomic)
                } else if FileManager.default.fileExists(atPath: url.path) {
                    try FileManager.default.removeItem(at: url)
                }
            }
        } catch { notice = error.localizedDescription; return }
        do { try await writeChoice(draft.selection.configValue) }
        catch { readFailure = error.localizedDescription; return }
        selection = draft.selection
        self.draft = nil
        await reload()
    }

    func cancel() {
        guard !saving else { return }
        draft = nil
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
