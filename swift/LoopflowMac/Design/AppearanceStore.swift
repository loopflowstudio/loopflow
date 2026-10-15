import AppKit
import Loopflow
import Observation

struct AppearanceSelection: Codable, Equatable {
    var mode: AppearanceMode = .system
    var lightThemeID = "light"
    var darkThemeID = "dark"

    func themeID(dark: Bool) -> String { dark ? darkThemeID : lightThemeID }
    func isDark(systemDark: Bool) -> Bool { mode == .dark || (mode == .system && systemDark) }
}

private struct AppearancePreferences: Codable {
    let version: Int
    let selection: AppearanceSelection
    let imports: [ImportedTheme]
}

/// All windows share two assigned themes, one mode and one reversible preview.
@MainActor @Observable
final class AppearanceStore {
    static let shared = AppearanceStore(publish: { try GhosttyManager.shared.applyTheme($0, retaining: $1) })
    private(set) var selection = AppearanceSelection()
    private var draft: AppearanceSelection?
    private(set) var editingSlot: AppearanceMode = .light
    private(set) var imports: [ImportedTheme] = []
    private var pendingImports: [ImportedTheme] = []
    private(set) var effectiveTheme = ThemeDefinition.light
    private(set) var systemDark: Bool
    var notice: String?
    @ObservationIgnored private let defaults: UserDefaults
    @ObservationIgnored private let publish: (ThemeDefinition, String) throws -> Void
    @ObservationIgnored private var observation: NSKeyValueObservation?

    init(defaults: UserDefaults = .standard, systemDark: Bool? = nil,
         publish: @escaping (ThemeDefinition, String) throws -> Void = { _, _ in }) {
        self.defaults = defaults
        self.publish = publish
        self.systemDark = systemDark ?? (NSApplication.shared.effectiveAppearance.bestMatch(from: [.darkAqua, .aqua]) == .darkAqua)
        var migrated = false
        if let data = defaults.data(forKey: "appearancePreferences") {
            do {
                let preferences = try Self.decode(data)
                guard preferences.imports.allSatisfy({ $0.terminal.indexed.count == 256 && $0.definition.readabilityErrors.isEmpty }) else {
                    throw ThemeError("Saved theme contains invalid colors.")
                }
                imports = preferences.imports
                guard resolve(preferences.selection.lightThemeID) != nil,
                      resolve(preferences.selection.darkThemeID) != nil else {
                    throw ThemeError("Saved theme is unavailable. Choose another theme or import it again.")
                }
                selection = preferences.selection
                migrated = preferences.version < 3
            } catch { notice = "\(error.localizedDescription) Saved preferences were retained; using System until a valid theme is applied." }
        } else if defaults.object(forKey: "appearancePreferences") != nil {
            notice = "Unreadable appearance preferences were retained. Choose a theme to replace them."
        } else if let old = defaults.string(forKey: "appearanceMode") {
            selection.mode = AppearanceMode(rawValue: old) ?? .system
            migrated = true
        }
        editingSlot = selection.isDark(systemDark: self.systemDark) ? .dark : .light
        effectiveTheme = resolved(selection)
        if AppTestMode.forcesLightAppearance { effectiveTheme = .light }
        do { try publish(effectiveTheme, effectiveTheme.id) } catch { notice = error.localizedDescription }
        if migrated { persist(); defaults.removeObject(forKey: "appearanceMode") }
        if systemDark == nil {
            observation = NSApplication.shared.observe(\.effectiveAppearance) { [weak self] _, _ in
                Task { @MainActor [weak self] in
                    self?.setSystemDark(NSApplication.shared.effectiveAppearance.bestMatch(from: [.darkAqua, .aqua]) == .darkAqua)
                }
            }
        }
    }

    var current: AppearanceSelection { draft ?? selection }
    var themes: [ThemeDefinition] { ThemeDefinition.builtins + (imports + pendingImports).map(\.definition) }
    var chooserOpen: Bool { draft != nil }
    var selectedID: String { current.themeID(dark: editingSlot == .dark) }
    var selectedTheme: ThemeDefinition { resolve(selectedID) ?? .light }
    var diagnostics: [String] { (imports + pendingImports).first { $0.id == selectedID }?.diagnostics ?? [] }
    var assignmentErrors: [String] {
        [AppearanceMode.light, .dark].flatMap { slot -> [String] in
            guard let theme = resolve(current.themeID(dark: slot == .dark)) else {
                return ["\(slot.menuTitle) theme is unavailable."]
            }
            return theme.readabilityErrors.map { "\(slot.menuTitle) theme: \($0)" }
        }
    }
    var canApply: Bool { assignmentErrors.isEmpty }
    func themeName(for slot: AppearanceMode) -> String {
        resolve(current.themeID(dark: slot == .dark))?.name ?? "Unavailable"
    }
    private func resolve(_ id: String) -> ThemeDefinition? {
        ThemeDefinition.builtins.first { $0.id == id } ?? (imports + pendingImports).first { $0.id == id }?.definition
    }
    private func resolved(_ choice: AppearanceSelection) -> ThemeDefinition {
        resolve(choice.themeID(dark: choice.isDark(systemDark: systemDark))) ?? (systemDark ? .dark : .light)
    }

    func setSystemDark(_ dark: Bool) {
        guard systemDark != dark else { return }
        systemDark = dark
        if draft == nil { _ = update(resolved(selection)) }
        else if current.mode == .system {
            editingSlot = dark ? .dark : .light
            _ = update(selectedTheme)
        }
    }
    func beginPreview() {
        guard draft == nil else { return }
        draft = selection
        editingSlot = selection.isDark(systemDark: systemDark) ? .dark : .light
    }
    func editSlot(_ slot: AppearanceMode) {
        guard slot != .system else { return }
        beginPreview()
        editingSlot = slot
        _ = update(selectedTheme)
    }
    func previewMode(_ mode: AppearanceMode) {
        beginPreview()
        draft?.mode = mode
        editingSlot = current.isDark(systemDark: systemDark) ? .dark : .light
        _ = update(selectedTheme)
    }
    func previewTheme(_ id: String) {
        guard let theme = resolve(id) else { notice = "Theme unavailable."; return }
        beginPreview()
        if editingSlot == .dark { draft?.darkThemeID = id } else { draft?.lightThemeID = id }
        _ = update(theme)
    }
    func importTheme(_ url: URL) {
        do {
            let imported = try GhosttyThemeImport.read(url)
            pendingImports.append(imported)
            previewTheme(imported.id)
        } catch { notice = error.localizedDescription }
    }
    func applyMode(_ mode: AppearanceMode) {
        var choice = selection
        choice.mode = mode
        guard update(resolved(choice)) else { return }
        selection = choice
        draft = nil; pendingImports = []
        persist()
    }
    func apply() {
        guard canApply, update(resolved(current)) else { return }
        selection = current
        let used = [selection.lightThemeID, selection.darkThemeID]
        imports += pendingImports.filter { used.contains($0.id) }
        pendingImports = []; draft = nil
        persist()
    }
    func cancel() {
        guard update(resolved(selection)) else { return }
        draft = nil; pendingImports = []
    }
    @discardableResult private func update(_ theme: ThemeDefinition) -> Bool {
        do {
            guard theme.readabilityErrors.isEmpty else { throw ThemeError(theme.readabilityErrors.joined(separator: "\n")) }
            let theme = AppTestMode.forcesLightAppearance ? ThemeDefinition.light : theme
            if effectiveTheme.id != theme.id || effectiveTheme.terminal != theme.terminal || effectiveTheme.palette.definition != theme.palette.definition { try publish(theme, resolved(selection).id) }
            effectiveTheme = theme; notice = nil
            return true
        } catch { notice = error.localizedDescription; return false }
    }
    private func persist() {
        do {
            let data = try JSONEncoder().encode(AppearancePreferences(version: 3, selection: selection, imports: imports))
            defaults.set(data, forKey: "appearancePreferences")
        } catch { notice = error.localizedDescription }
    }

    private static func decode(_ data: Data) throws -> AppearancePreferences {
        struct Version: Decodable { let version: Int }
        struct PreviousImport: Decodable {
            let id: String
            let name: String
            let terminal: TerminalPalette
            let diagnostics: [String]
            func converted() throws -> ImportedTheme {
                guard terminal.indexed.count == 256 else { throw ThemeError("Saved theme has an incomplete terminal palette.") }
                return ImportedTheme(id: id, name: name, terminal: terminal, diagnostics: diagnostics,
                              palette: GhosttyThemeMapping.palette(terminal))
            }
        }
        let decoder = JSONDecoder()
        switch try decoder.decode(Version.self, from: data).version {
        case 3: return try decoder.decode(AppearancePreferences.self, from: data)
        case 2:
            struct Previous: Decodable { let selection: AppearanceSelection; let imports: [PreviousImport] }
            let previous = try decoder.decode(Previous.self, from: data)
            return AppearancePreferences(version: 2, selection: previous.selection, imports: try previous.imports.map { try $0.converted() })
        case 1:
            struct Previous: Decodable { let selection: String; let imports: [PreviousImport] }
            let previous = try decoder.decode(Previous.self, from: data)
            var choice = AppearanceSelection()
            if let mode = AppearanceMode(rawValue: previous.selection) { choice.mode = mode }
            else if let imported = previous.imports.first(where: { $0.id == previous.selection }) {
                choice.mode = try imported.converted().definition.isDark ? .dark : .light
                if choice.mode == .dark { choice.darkThemeID = imported.id } else { choice.lightThemeID = imported.id }
            } else { throw ThemeError("Saved theme is unavailable.") }
            return AppearancePreferences(version: 1, selection: choice, imports: try previous.imports.map { try $0.converted() })
        default: throw ThemeError("Unsupported appearance preferences version.")
        }
    }
}
