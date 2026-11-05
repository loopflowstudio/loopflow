import Foundation
import Loopflow

/// Where themes and the `theme:` choice live. Loopflow writes only `themes` and the config's `theme:` line.
struct ThemePaths: Equatable {
    var config: URL
    var themes: URL
    var ghostty: URL?

    static var standard: Self {
        let environment = ProcessInfo.processInfo.environment
        let home = URL(fileURLWithPath: NSHomeDirectory(), isDirectory: true)
        let lf = environment["LF_HOME"].flatMap { $0.isEmpty ? nil : URL(fileURLWithPath: $0, isDirectory: true) }
            ?? home.appendingPathComponent(".lf", isDirectory: true)
        let xdg = environment["XDG_CONFIG_HOME"].flatMap { $0.isEmpty ? nil : URL(fileURLWithPath: $0, isDirectory: true) }
            ?? home.appendingPathComponent(".config", isDirectory: true)
        return Self(config: lf.appendingPathComponent("config.yaml"),
                    themes: lf.appendingPathComponent("themes", isDirectory: true),
                    ghostty: xdg.appendingPathComponent("ghostty/themes", isDirectory: true))
    }
}

/// One theme a picker can show, with where it came from.
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

    /// One appearance's picker: built-ins (yours where you override one), then yours and Ghostty's.
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

/// `theme:` in `~/.lf/config.yaml`, in Ghostty's value syntax: `light:A,dark:B` or one name.
enum ThemeConfig {
    enum Value: Equatable {
        case single(String)
        case pair(light: String?, dark: String?)
    }

    static func parse(_ text: String) -> Value? {
        let text = text.trimmingCharacters(in: .whitespaces)
        guard !text.isEmpty else { return nil }
        var light: String?, dark: String?
        for part in text.split(separator: ",") {
            let pair = part.split(separator: ":", maxSplits: 1).map { $0.trimmingCharacters(in: .whitespaces) }
            switch (pair.first, pair.count) {
            case ("light", 2): light = pair[1]
            case ("dark", 2): dark = pair[1]
            default: return .single(text)
            }
        }
        return .pair(light: light, dark: dark)
    }

    /// Read and edit the same typed declaration as the CLI.
    static func read(_ url: URL) -> String? {
        guard let data = try? LoopflowLibrary.configuration(request(url, value: nil)),
              let result = try? JSONDecoder().decode(ConfigurationFileValue.self, from: data),
              case .string(let value) = result.value else { return nil }
        return value
    }

    static func write(_ value: String, to url: URL) throws {
        let encoded = String(decoding: try JSONEncoder().encode(value), as: UTF8.self)
        _ = try LoopflowLibrary.configuration(request(url, value: encoded))
    }

    private static func request(_ url: URL, value: String?) -> [String: Any] {
        ["action": "theme", "path": url.path, "value": value as Any? ?? NSNull()]
    }

}
