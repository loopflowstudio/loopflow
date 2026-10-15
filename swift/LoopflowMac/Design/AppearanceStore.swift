import AppKit
import Loopflow
import Observation

private struct AppearancePreferences: Codable {
    let version: Int
    let selection: String
    let imports: [ImportedTheme]
}

/// All windows share one selection and one reversible preview transaction.
@MainActor @Observable
final class AppearanceStore {
    static let shared = AppearanceStore(publish: { try GhosttyManager.shared.applyTheme($0, retaining: $1) })
    private(set) var selection = "system"
    private var previewSelection: String?
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
        if let data = defaults.data(forKey: "appearancePreferences") {
            do {
                let preferences = try JSONDecoder().decode(AppearancePreferences.self, from: data)
                guard preferences.version == 1 else { throw ThemeError("Unsupported appearance preferences version.") }
                guard preferences.imports.allSatisfy({ $0.terminal.indexed.count == 256 && $0.terminal.readabilityErrors.isEmpty }) else {
                    throw ThemeError("Saved theme contains invalid colors.")
                }
                selection = preferences.selection; imports = preferences.imports
                guard resolve(selection) != nil else { throw ThemeError("Saved theme is unavailable. Choose System, Light, Dark or import it again.") }
            } catch { notice = "\(error.localizedDescription) Saved preferences were retained; using System until a valid theme is applied." }
        } else if defaults.object(forKey: "appearancePreferences") != nil {
            notice = "Unreadable appearance preferences were retained. Choose a theme to replace them."
        } else if let old = defaults.string(forKey: "appearanceMode") {
            selection = ["system", "light", "dark"].contains(old) ? old : "system"
            persist()
            defaults.removeObject(forKey: "appearanceMode")
        }
        effectiveTheme = resolve(selection) ?? systemTheme
        if AppTestMode.forcesLightAppearance { effectiveTheme = .light }
        do { try publish(effectiveTheme, effectiveTheme.id) } catch { notice = error.localizedDescription }
        if systemDark == nil {
            observation = NSApplication.shared.observe(\.effectiveAppearance) { [weak self] _, _ in
                Task { @MainActor [weak self] in
                    self?.setSystemDark(NSApplication.shared.effectiveAppearance.bestMatch(from: [.darkAqua, .aqua]) == .darkAqua)
                }
            }
        }
    }

    var themes: [ThemeDefinition] { ThemeDefinition.builtins + (imports + pendingImports).map(\.definition) }
    var chooserOpen: Bool { previewSelection != nil }
    var selectedID: String { previewSelection ?? selection }
    var selectedTheme: ThemeDefinition { resolve(selectedID) ?? systemTheme }
    var diagnostics: [String] { (imports + pendingImports).first { $0.id == selectedID }?.diagnostics ?? [] }
    private var systemTheme: ThemeDefinition { systemDark ? .dark : .light }
    private func resolve(_ id: String) -> ThemeDefinition? {
        if id == "system" { return systemTheme }
        if let builtin = ThemeDefinition.builtins.first(where: { $0.id == id }) { return builtin }
        // Derive only the selected import; the chooser resolves the full catalogue.
        return (imports + pendingImports).first { $0.id == id }?.definition
    }

    func setSystemDark(_ dark: Bool) {
        systemDark = dark
        if selectedID == "system" || resolve(selectedID) == nil { _ = update(systemTheme) }
    }
    func beginPreview() { if !chooserOpen { previewSelection = selection } }
    func preview(_ id: String) {
        guard let theme = resolve(id) else { notice = "Theme unavailable."; return }
        previewSelection = id
        _ = update(theme)
    }
    func importTheme(_ url: URL) {
        do {
            let imported = try GhosttyThemeImport.read(url)
            pendingImports.append(imported)
            preview(imported.id)
        } catch { notice = error.localizedDescription }
    }
    func apply(_ id: String? = nil) {
        let id = id ?? selectedID
        guard let theme = resolve(id), update(theme) else { return }
        selection = id
        imports += pendingImports.filter { $0.id == id }
        pendingImports = []; previewSelection = nil
        persist()
    }
    func cancel() {
        guard update(resolve(selection) ?? systemTheme) else { return }
        previewSelection = nil; pendingImports = []
    }
    @discardableResult private func update(_ theme: ThemeDefinition) -> Bool {
        do {
            guard theme.terminal.readabilityErrors.isEmpty else { throw ThemeError(theme.terminal.readabilityErrors.joined(separator: "\n")) }
            let theme = AppTestMode.forcesLightAppearance ? ThemeDefinition.light : theme
            if effectiveTheme.id != theme.id || effectiveTheme.terminal != theme.terminal { try publish(theme, (resolve(selection) ?? systemTheme).id) }
            effectiveTheme = theme; notice = nil
            return true
        } catch { notice = error.localizedDescription; return false }
    }
    private func persist() {
        do {
            let data = try JSONEncoder().encode(AppearancePreferences(version: 1, selection: selection, imports: imports))
            defaults.set(data, forKey: "appearancePreferences")
        } catch { notice = error.localizedDescription }
    }
}
