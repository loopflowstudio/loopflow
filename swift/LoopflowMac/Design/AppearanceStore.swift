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
    var refusal: String?
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
    private(set) var picker: AppearanceMode = .light
    private(set) var effectiveTheme = ThemeDefinition.light
    private(set) var systemDark: Bool
    private(set) var diagnostics: [String] = []
    var notice: String?
    @ObservationIgnored private let publish: (ThemeDefinition, String) throws -> Void
    @ObservationIgnored private var observation: NSKeyValueObservation?
    @ObservationIgnored private var watcher: Timer?
    @ObservationIgnored private var fingerprint: [String] = []

    init(paths: ThemePaths, systemDark: Bool? = nil, watches: Bool = false,
         publish: @escaping (ThemeDefinition, String) throws -> Void = { _, _ in }) {
        self.paths = paths
        self.publish = publish
        self.systemDark = systemDark ?? (NSApplication.shared.effectiveAppearance.bestMatch(from: [.darkAqua, .aqua]) == .darkAqua)
        reload()
        do { try publish(effectiveTheme, effectiveTheme.id) } catch { notice = error.localizedDescription }
        if systemDark == nil {
            observation = NSApplication.shared.observe(\.effectiveAppearance) { [weak self] _, _ in
                Task { @MainActor [weak self] in
                    self?.setSystemDark(NSApplication.shared.effectiveAppearance.bestMatch(from: [.darkAqua, .aqua]) == .darkAqua)
                }
            }
        }
        if watches {
            fingerprint = currentFingerprint()
            watcher = Timer.scheduledTimer(withTimeInterval: 1, repeats: true) { [weak self] _ in
                Task { @MainActor [weak self] in self?.reloadIfChanged() }
            }
        }
    }

    // MARK: Reading

    var current: AppearanceSelection { draft?.selection ?? selection }
    private var currentLibrary: ThemeLibrary { draft?.library ?? library }
    var chooserOpen: Bool { draft != nil }
    var selectedName: String { current.name(dark: picker == .dark) }
    var selectedTheme: ThemeDefinition { theme(selectedName, dark: picker == .dark, in: currentLibrary) }
    var selectedSource: ThemeEntry.Source? { currentLibrary.source(of: selectedName) }
    func entries(for side: AppearanceMode) -> (builtins: [ThemeEntry], yours: [ThemeEntry]) {
        currentLibrary.entries(dark: side == .dark)
    }
    var canDelete: Bool { selectedSource == .yours }
    var deleteRevertsToBuiltin: Bool { canDelete && currentLibrary.isBuiltin(selectedName) }
    var refusal: String? { draft?.refusal }
    var draftErrors: [String] {
        guard let draft else { return [] }
        return draft.changed.sorted().flatMap { name in
            (draft.library.yours[name]?.readabilityErrors ?? []).map { "\(name): \($0)" }
        }
    }
    var canApply: Bool { refusal == nil && draftErrors.isEmpty }
    func themeName(for picker: AppearanceMode) -> String { current.name(dark: picker == .dark) }

    private func theme(_ name: String, dark: Bool, in library: ThemeLibrary) -> ThemeDefinition {
        library.resolve(name) ?? (dark ? .dark : .light)
    }
    private func resolved(_ choice: AppearanceSelection, in library: ThemeLibrary) -> ThemeDefinition {
        let dark = choice.isDark(systemDark: systemDark)
        return theme(choice.name(dark: dark), dark: dark, in: library)
    }

    /// Read your files and `theme:` again. An unknown name keeps the last good choice.
    func reload() {
        library = ThemeLibrary.load(paths)
        var problems = library.problems
        switch selection(from: ThemeConfig.read(paths.config)) {
        case let .success(choice): selection = choice
        case let .failure(error): problems.insert(error.message, at: 0)
        }
        notice = problems.isEmpty ? nil : problems.joined(separator: "\n")
        if draft == nil {
            picker = selection.isDark(systemDark: systemDark) ? .dark : .light
            update(resolved(selection, in: library), keepingNotice: true)
        }
    }

    private func selection(from raw: String?) -> Result<AppearanceSelection, ThemeError> {
        guard let raw, let value = ThemeConfig.parse(raw) else { return .success(AppearanceSelection()) }
        let config = paths.config.path
        func unknown(_ name: String) -> ThemeError {
            ThemeError("No theme named “\(name)”: keeping \(effectiveTheme.name). Fix theme: in \(config) with a built-in name or a file in \(paths.themes.path).")
        }
        var choice = selection
        switch value {
        case let .single(name):
            guard let theme = library.resolve(name) else { return .failure(unknown(name)) }
            choice.mode = theme.isDark ? .dark : .light
            if theme.isDark { choice.dark = name } else { choice.light = name }
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
        let files = [paths.config] + [paths.themes, paths.ghostty].compactMap { $0 }.flatMap { (try? ThemeFile.directory($0)) ?? [] }
        return files.map { url in
            let values = try? url.resourceValues(forKeys: [.contentModificationDateKey, .fileSizeKey])
            return "\(url.path)|\(values?.contentModificationDate?.timeIntervalSince1970 ?? 0)|\(values?.fileSize ?? -1)"
        }
    }

    private func reloadIfChanged() {
        let latest = currentFingerprint()
        guard latest != fingerprint else { return }
        fingerprint = latest
        reload()
    }

    // MARK: Preview

    func setSystemDark(_ dark: Bool) {
        guard systemDark != dark else { return }
        systemDark = dark
        if draft == nil { update(resolved(selection, in: library)) }
        else if current.mode == .system {
            picker = dark ? .dark : .light
            update(selectedTheme)
        }
    }
    func beginPreview() {
        guard draft == nil else { return }
        draft = AppearanceDraft(selection: selection, library: library)
        picker = selection.isDark(systemDark: systemDark) ? .dark : .light
        diagnostics = []
    }
    func showPicker(_ appearance: AppearanceMode) {
        guard appearance != .system else { return }
        beginPreview()
        picker = appearance
        draft?.refusal = nil
        update(selectedTheme)
    }
    func previewMode(_ mode: AppearanceMode) {
        beginPreview()
        draft?.selection.mode = mode
        draft?.refusal = nil
        picker = current.isDark(systemDark: systemDark) ? .dark : .light
        update(selectedTheme)
    }
    func previewTheme(_ name: String) {
        guard let theme = currentLibrary.resolve(name) else { notice = "No theme named “\(name)”."; return }
        beginPreview()
        if picker == .dark { draft?.selection.dark = name } else { draft?.selection.light = name }
        draft?.refusal = nil
        diagnostics = []
        update(theme)
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
            if picker == .dark { draft?.selection.dark = ThemeDefinition.dark.name } else { draft?.selection.light = ThemeDefinition.light.name }
        }
        update(selectedTheme)
    }

    /// Import a theme file into the active picker. Only that picker's appearance is accepted.
    func importTheme(_ url: URL) {
        beginPreview()
        do {
            let file = try ThemeFile.read(url)
            let dark = picker == .dark
            guard file.theme.isDark == dark else {
                let other = file.theme.isDark ? "Dark" : "Light"
                draft?.refusal = "“\(file.theme.name)” is a \(other.lowercased()) theme. Import it from the \(other) picker."
                return
            }
            draft?.refusal = nil
            stage(file.theme, for: file.theme.name)
            if dark { draft?.selection.dark = file.theme.name } else { draft?.selection.light = file.theme.name }
            diagnostics = file.diagnostics
            update(file.theme)
        } catch { notice = error.localizedDescription }
    }

    /// Stage your file `name`: a theme to write, or nil to delete.
    private func stage(_ theme: ThemeDefinition?, for name: String) {
        draft?.changed.insert(name)
        draft?.library.yours[name] = theme
        if let theme { update(theme) }
    }

    // MARK: Writing

    /// Menu Mode change: writes the config at once and discards any preview.
    func applyMode(_ mode: AppearanceMode) {
        var choice = selection
        choice.mode = mode
        do {
            try ThemeConfig.write(choice.configValue, to: paths.config)
            selection = choice
            draft = nil
            update(resolved(selection, in: library))
        } catch { notice = error.localizedDescription }
    }

    func apply() {
        guard let draft, canApply else { return }
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
            try ThemeConfig.write(draft.selection.configValue, to: paths.config)
        } catch { notice = error.localizedDescription; return }
        selection = draft.selection
        self.draft = nil
        reload()
    }

    func cancel() {
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
