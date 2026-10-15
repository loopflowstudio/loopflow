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
    let overrides: [ThemeRecord]
}

private struct AppearanceDraft {
    var selection: AppearanceSelection
    var overrides: [ThemeRecord]
}

/// All windows share a fixed palette library, two assignments and one reversible preview.
@MainActor @Observable
final class AppearanceStore {
    static let shared = AppearanceStore(publish: { try GhosttyManager.shared.applyTheme($0, retaining: $1) })
    private(set) var selection = AppearanceSelection()
    private(set) var overrides: [ThemeRecord] = []
    private var draft: AppearanceDraft?
    private(set) var editingSlot: AppearanceMode = .light
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
                let ids = Set(ThemeDefinition.builtins.map(\.id))
                guard preferences.overrides.count <= ids.count,
                      Set(preferences.overrides.map(\.id)).count == preferences.overrides.count,
                      preferences.overrides.allSatisfy({ ids.contains($0.id) && Self.errors($0).isEmpty }),
                      ids.contains(preferences.selection.lightThemeID), ids.contains(preferences.selection.darkThemeID) else {
                    throw ThemeError("Saved theme library contains unavailable slots or invalid colors.")
                }
                overrides = preferences.overrides
                selection = preferences.selection
                migrated = preferences.version < 5
            } catch { notice = "\(error.localizedDescription) Saved preferences were retained; using System until a valid theme is applied." }
        } else if defaults.object(forKey: "appearancePreferences") != nil {
            notice = "Unreadable appearance preferences were retained. Choose a theme to replace them."
        } else if let old = defaults.string(forKey: "appearanceMode") {
            selection.mode = AppearanceMode(rawValue: old) ?? .system
            migrated = true
        }
        editingSlot = selection.isDark(systemDark: self.systemDark) ? .dark : .light
        effectiveTheme = resolved(selection, overrides: overrides)
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

    var current: AppearanceSelection { draft?.selection ?? selection }
    private var currentOverrides: [ThemeRecord] { draft?.overrides ?? overrides }
    var themes: [ThemeDefinition] {
        ThemeDefinition.builtins.map { builtin in currentOverrides.first { $0.id == builtin.id }?.definition ?? builtin }
    }
    var chooserOpen: Bool { draft != nil }
    var selectedID: String { current.themeID(dark: editingSlot == .dark) }
    var selectedTheme: ThemeDefinition { resolve(selectedID) ?? .light }
    var selectedRecord: ThemeRecord {
        currentOverrides.first { $0.id == selectedID } ?? Self.record(selectedTheme)
    }
    var originalTheme: ThemeDefinition { ThemeDefinition.builtins.first { $0.id == selectedID } ?? .light }
    var canReset: Bool { currentOverrides.contains { $0.id == selectedID } }
    var diagnostics: [String] { selectedRecord.diagnostics }
    var libraryErrors: [String] { currentOverrides.flatMap(Self.errors) }
    var canApply: Bool { libraryErrors.isEmpty }
    func themeName(for slot: AppearanceMode) -> String {
        resolve(current.themeID(dark: slot == .dark))?.name ?? "Unavailable"
    }
    private func resolve(_ id: String) -> ThemeDefinition? { themes.first { $0.id == id } }
    private func resolved(_ choice: AppearanceSelection, overrides: [ThemeRecord]) -> ThemeDefinition {
        let id = choice.themeID(dark: choice.isDark(systemDark: systemDark))
        return overrides.first { $0.id == id }?.definition ?? ThemeDefinition.builtins.first { $0.id == id } ?? .light
    }
    private static func record(_ theme: ThemeDefinition) -> ThemeRecord {
        ThemeRecord(id: theme.id, name: theme.name, terminal: theme.terminal, diagnostics: [], palette: theme.palette.definition)
    }
    private static func errors(_ theme: ThemeRecord) -> [String] {
        (theme.name.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty ? ["Theme name cannot be empty."] : []) +
        theme.definition.readabilityErrors.map { "\(theme.name): \($0)" }
    }

    func setSystemDark(_ dark: Bool) {
        guard systemDark != dark else { return }
        systemDark = dark
        if draft == nil { _ = update(resolved(selection, overrides: overrides)) }
        else if current.mode == .system {
            editingSlot = dark ? .dark : .light
            _ = update(selectedTheme)
        }
    }
    func beginPreview() {
        guard draft == nil else { return }
        draft = AppearanceDraft(selection: selection, overrides: overrides)
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
        draft?.selection.mode = mode
        editingSlot = current.isDark(systemDark: systemDark) ? .dark : .light
        _ = update(selectedTheme)
    }
    func previewTheme(_ id: String) {
        guard let theme = resolve(id) else { notice = "Theme unavailable."; return }
        beginPreview()
        if editingSlot == .dark { draft?.selection.darkThemeID = id } else { draft?.selection.lightThemeID = id }
        _ = update(theme)
    }
    func editTheme(_ edit: (inout ThemeRecord) -> Void) {
        beginPreview()
        var record = selectedRecord
        edit(&record)
        replaceSelected(with: record)
    }
    private func replaceSelected(with record: ThemeRecord) {
        let replacement = ThemeRecord(id: selectedID, name: record.name, terminal: record.terminal,
                                      diagnostics: record.diagnostics, palette: record.palette)
        let id = selectedID
        draft?.overrides.removeAll { $0.id == id }
        draft?.overrides.append(replacement)
        _ = update(replacement.definition)
    }
    func resetTheme() {
        beginPreview()
        let id = selectedID
        draft?.overrides.removeAll { $0.id == id }
        _ = update(selectedTheme)
    }
    func importTheme(_ url: URL, replacing id: String? = nil) {
        do {
            let imported = try ThemeImport.read(url)
            guard id == nil || selectedID == id else { throw ThemeError("The selected theme changed. Import again into the intended slot.") }
            beginPreview()
            replaceSelected(with: imported)
        } catch { notice = error.localizedDescription }
    }
    func applyMode(_ mode: AppearanceMode) {
        var choice = selection
        choice.mode = mode
        guard update(resolved(choice, overrides: overrides)) else { return }
        selection = choice
        draft = nil
        persist()
    }
    func apply() {
        guard canApply, update(resolved(current, overrides: currentOverrides)) else { return }
        selection = current
        overrides = currentOverrides
        draft = nil
        persist()
    }
    func cancel() {
        guard update(resolved(selection, overrides: overrides)) else { return }
        draft = nil
    }
    @discardableResult private func update(_ theme: ThemeDefinition) -> Bool {
        do {
            guard theme.readabilityErrors.isEmpty else { throw ThemeError(theme.readabilityErrors.joined(separator: "\n")) }
            let theme = AppTestMode.forcesLightAppearance ? ThemeDefinition.light : theme
            if effectiveTheme.id != theme.id || effectiveTheme.terminal != theme.terminal || effectiveTheme.palette.definition != theme.palette.definition {
                try publish(theme, resolved(selection, overrides: overrides).id)
            }
            effectiveTheme = theme; notice = nil
            return true
        } catch { notice = error.localizedDescription; return false }
    }
    private func persist() {
        do {
            let data = try JSONEncoder().encode(AppearancePreferences(version: 5, selection: selection, overrides: overrides))
            defaults.set(data, forKey: "appearancePreferences")
        } catch { notice = error.localizedDescription }
    }

    private static func decode(_ data: Data) throws -> AppearancePreferences {
        struct Version: Decodable { let version: Int }
        struct Previous: Decodable { let selection: AppearanceSelection; let imports: [ThemeRecord] }
        struct PreviousImport: Decodable {
            let id: String
            let name: String
            let terminal: TerminalPalette
            let diagnostics: [String]
            func converted() throws -> ThemeRecord {
                guard terminal.indexed.count == 256 else { throw ThemeError("Saved theme has an incomplete terminal palette.") }
                return ThemeRecord(id: id, name: name, terminal: terminal, diagnostics: diagnostics,
                                   palette: GhosttyThemeMapping.palette(terminal))
            }
        }
        let decoder = JSONDecoder()
        let version = try decoder.decode(Version.self, from: data).version
        let previous: Previous
        switch version {
        case 5: return try decoder.decode(AppearancePreferences.self, from: data)
        case 4: previous = try decoder.decode(Previous.self, from: data)
        case 3:
            guard var record = try JSONSerialization.jsonObject(with: data) as? [String: Any],
                  var imports = record["imports"] as? [[String: Any]] else {
                throw ThemeError("Unreadable appearance preferences.")
            }
            for index in imports.indices {
                guard var palette = imports[index]["palette"] as? [String: Any],
                      let ink = palette["emphasisInk"], let pressed = palette["accentHover"] else {
                    throw ThemeError("Saved theme has an incomplete app palette.")
                }
                palette["accentInk"] = ink; palette["accentPressed"] = pressed
                for key in ["terminalHover", "terminalSelection", "terminalFailure"] { palette.removeValue(forKey: key) }
                imports[index]["palette"] = palette
            }
            record["imports"] = imports
            previous = try decoder.decode(Previous.self, from: JSONSerialization.data(withJSONObject: record))
        case 2:
            struct Old: Decodable { let selection: AppearanceSelection; let imports: [PreviousImport] }
            let old = try decoder.decode(Old.self, from: data)
            previous = Previous(selection: old.selection, imports: try old.imports.map { try $0.converted() })
        case 1:
            struct Old: Decodable { let selection: String; let imports: [PreviousImport] }
            let old = try decoder.decode(Old.self, from: data)
            let imports = try old.imports.map { try $0.converted() }
            var choice = AppearanceSelection()
            if let mode = AppearanceMode(rawValue: old.selection) { choice.mode = mode }
            else if let imported = imports.first(where: { $0.id == old.selection }) {
                choice.mode = imported.definition.isDark ? .dark : .light
                if choice.mode == .dark { choice.darkThemeID = imported.id } else { choice.lightThemeID = imported.id }
            } else { throw ThemeError("Saved theme is unavailable.") }
            previous = Previous(selection: choice, imports: imports)
        default: throw ThemeError("Unsupported appearance preferences version.")
        }
        var choice = previous.selection
        var records = previous.imports
        for id in Set([choice.lightThemeID, choice.darkThemeID]) where !records.contains(where: { $0.id == id }) {
            if let retired = try ThemeDefinition.retiredBuiltin(id) { records.append(record(retired)) }
        }
        let selected = Set([choice.lightThemeID, choice.darkThemeID])
        var available = ThemeDefinition.builtins.filter { !selected.contains($0.id) }.map(\.id)
        var mapped: [String: String] = [:]
        var overrides: [ThemeRecord] = []
        for record in records {
            guard !available.isEmpty else { throw ThemeError("The previous library cannot fit in 24 slots without losing themes. Its preferences were retained.") }
            let id = available.removeFirst()
            mapped[record.id] = id
            overrides.append(ThemeRecord(id: id, name: record.name, terminal: record.terminal,
                                         diagnostics: record.diagnostics, palette: record.palette))
        }
        choice.lightThemeID = mapped[choice.lightThemeID] ?? choice.lightThemeID
        choice.darkThemeID = mapped[choice.darkThemeID] ?? choice.darkThemeID
        return AppearancePreferences(version: version, selection: choice, overrides: overrides)
    }
}
