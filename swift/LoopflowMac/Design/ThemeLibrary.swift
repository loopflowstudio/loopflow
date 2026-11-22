import Foundation
import Loopflow

/// Where theme files live; Loopflow writes only `themes`. The `theme:` choice is the server's.
struct ThemePaths: Equatable {
    var themes: URL
    var ghostty: URL?

    static var standard: Self {
        let environment = ProcessInfo.processInfo.environment
        let home = URL(fileURLWithPath: NSHomeDirectory(), isDirectory: true)
        let lf = environment["LF_HOME"].flatMap { $0.isEmpty ? nil : URL(fileURLWithPath: $0, isDirectory: true) }
            ?? home.appendingPathComponent(".lf", isDirectory: true)
        let xdg = environment["XDG_CONFIG_HOME"].flatMap { $0.isEmpty ? nil : URL(fileURLWithPath: $0, isDirectory: true) }
            ?? home.appendingPathComponent(".config", isDirectory: true)
        return Self(themes: lf.appendingPathComponent("themes", isDirectory: true),
                    ghostty: xdg.appendingPathComponent("ghostty/themes", isDirectory: true))
    }
}

/// One theme the chooser can show, with where it came from.
struct ThemeEntry: Identifiable {
    enum Source: Equatable { case builtin, yours, ghostty }
    let theme: ThemeDefinition
    let source: Source
    var id: String { theme.name }
    var name: String { theme.name }
}

/// Built-ins, your files in `~/.lf/themes`, then Ghostty's theme folder. Names resolve in that
/// order, except that your file named like a built-in overrides it, as Ghostty's user folder does.
struct ThemeLibrary {
    var yours: [String: ThemeDefinition] = [:]
    var ghostty: [String: ThemeDefinition] = [:]
    var problems: [String] = []

    static func load(_ paths: ThemePaths) -> Self {
        var library = Self()
        do { _ = try ThemeFile.migrateRetiredRoles(in: paths.themes) }
        catch { library.problems.append("\(paths.themes.path): \(error.localizedDescription)") }
        library.yours = read(paths.themes, problems: &library.problems)
        if let ghostty = paths.ghostty { library.ghostty = read(ghostty, problems: &library.problems) }
        return library
    }

    private static func read(_ directory: URL, problems: inout [String]) -> [String: ThemeDefinition] {
        guard let files = try? ThemeFile.directory(directory) else { return [:] }
        var themes: [String: ThemeDefinition] = [:]
        for url in files {
            do { themes[url.lastPathComponent] = try ThemeFile.read(url).theme }
            catch { problems.append("\(url.path): \(error.localizedDescription)") }
        }
        return themes
    }

    func resolve(_ name: String) -> ThemeDefinition? {
        yours[name] ?? ThemeDefinition.builtins.first { $0.name == name } ?? ghostty[name]
    }

    func isBuiltin(_ name: String) -> Bool { ThemeDefinition.builtins.contains { $0.name == name } }

    /// One side's themes: built-ins (yours where you override one), then yours and Ghostty's.
    func entries(dark: Bool) -> (builtins: [ThemeEntry], yours: [ThemeEntry]) {
        let builtins = ThemeDefinition.builtins.filter { $0.isDark == dark }.map { theme in
            yours[theme.name].map { ThemeEntry(theme: $0, source: .yours) } ?? ThemeEntry(theme: theme, source: .builtin)
        }
        let own = yours.values.filter { !isBuiltin($0.name) }.map { ThemeEntry(theme: $0, source: .yours) }
        let ghostty = ghostty.values.filter { yours[$0.name] == nil && !isBuiltin($0.name) }
            .map { ThemeEntry(theme: $0, source: .ghostty) }
        let rest = (own + ghostty).filter { $0.theme.isDark == dark }.sorted { $0.name < $1.name }
        return (builtins, rest)
    }

    func source(of name: String) -> ThemeEntry.Source? {
        if yours[name] != nil { return .yours }
        if isBuiltin(name) { return .builtin }
        return ghostty[name] == nil ? nil : .ghostty
    }
}
