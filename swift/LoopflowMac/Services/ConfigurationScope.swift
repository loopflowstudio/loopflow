import Foundation
import Loopflow

// Rust derives paths and applicability from the destination.
enum ConfigurationDestination: String, CaseIterable, Identifiable {
    case personalGlobal = "personal_global"
    case personalRepository = "personal_repo"
    case sharedRepository = "shared_repo"
    case personalWave = "personal_wave"
    case sharedWave = "shared_wave"

    var id: String { rawValue }
    var isWave: Bool { self == .personalWave || self == .sharedWave }
    var title: String {
        switch self {
        case .personalGlobal: "My global preferences"
        case .personalRepository: "My repository settings"
        case .sharedRepository: "Shared repository settings"
        case .personalWave: "My Wave preferences"
        case .sharedWave: "Shared Wave preferences"
        }
    }

    func address(repository: String, wave: String) -> JSONValue {
        var value: [String: JSONValue] = ["kind": .string(rawValue)]
        if self != .personalGlobal { value["repo"] = .string(repository) }
        if isWave { value["wave"] = .string(wave) }
        return .object(value)
    }
}

struct ConfigurationDefinition: Decodable {
    /// The canonical key on its primitive; `shorthands` are the other spellings lf accepts.
    let key: String
    let shorthands: [String]
    let category: String
    let schema: JSONValue
    let `default`: String
    let applicability: String
    let combination: String
    let portability: String
    let lifecycle: String
    let description: String
}

struct ConfigurationInspection: Decodable {
    let skill: String?
    let occurrence: JSONValue?
    let settings: [String: ConfigurationValue]
}

struct ConfigurationScope: Codable {
    let settings: [String: ConfigurationValue]
    let files: [File]

    struct File: Codable, Identifiable {
        let destination: JSONValue
        let path: String
        let slots: [String: Slot]
        var id: JSONValue { destination }

        var hasOverrides: Bool {
            owner != .personalGlobal && slots.values.contains { !$0.entries.isEmpty }
        }

        func prepareForOpening() throws -> URL {
            let url = URL(fileURLWithPath: path)
            if !FileManager.default.fileExists(atPath: path) {
                // The scoped writer also installs the Git exclusion for a private checkout file.
                guard owner == .personalGlobal else {
                    throw RegistryQueryError("Save a setting here to create \(path).")
                }
                try FileManager.default.createDirectory(at: url.deletingLastPathComponent(), withIntermediateDirectories: true)
                do { try Data("{}\n".utf8).write(to: url, options: .withoutOverwriting) }
                catch CocoaError.fileWriteFileExists { /* Another editor created it. */ }
            }
            return url
        }

        var owner: ConfigurationDestination? {
            guard case .string(let kind) = destination["kind"] else { return nil }
            return ConfigurationDestination(rawValue: kind)
        }
    }

    struct Slot: Codable {
        let key: String
        let declared: Authored
        let entries: [ConfigurationEntry]
        let promotion: JSONValue
    }

    enum Authored: Codable, Hashable {
        case absent
        case present(JSONValue)

        private enum Keys: String, CodingKey { case kind, value }
        private enum Kind: String, Codable { case absent, present }

        init(from decoder: Decoder) throws {
            let fields = try decoder.container(keyedBy: Keys.self)
            switch try fields.decode(Kind.self, forKey: .kind) {
            case .absent: self = .absent
            case .present: self = .present(try fields.decode(JSONValue.self, forKey: .value))
            }
        }

        func encode(to encoder: Encoder) throws {
            var fields = encoder.container(keyedBy: Keys.self)
            switch self {
            case .absent: try fields.encode(Kind.absent, forKey: .kind)
            case .present(let value):
                try fields.encode(Kind.present, forKey: .kind)
                try fields.encode(value, forKey: .value)
            }
        }

        var value: JSONValue? {
            if case .present(let value) = self { return value }
            return nil
        }
    }

    func source(_ key: String) -> String? {
        let locators = Set(settings[key]?.candidates.filter { $0.reason == "selected" }.map(\.locator) ?? [])
        guard locators.count == 1, let path = locators.first else { return nil }
        if path == "builtin" { return "Default" }
        return files.first { file in
            if file.owner == .personalWave { return path.hasPrefix(file.path + "#waves.") }
            return file.path == path
        }?.owner?.title ?? path
    }
}

extension JSONValue {
    var settingsText: String {
        switch self {
        case .null: return "None"
        case .string(let text): return text
        case .bool(let enabled): return enabled ? "On" : "Off"
        case .integer, .number: return settingsYAML
        case .array(let items):
            if items.isEmpty { return "Empty list" }
            return items.allSatisfy(\.isScalar) ? items.map(\.settingsText).joined(separator: ", ") : settingsYAML
        case .object(let fields):
            if fields.isEmpty { return "Empty" }
            let declared = fields.filter { $0.value != .null }.sorted { $0.key < $1.key }
            if declared.isEmpty { return "None" }
            guard declared.allSatisfy({ $0.value.isScalar }) else { return settingsYAML }
            return declared.map { "\($0.key.replacingOccurrences(of: "_", with: " ").capitalized): \($0.value.settingsText)" }
                .joined(separator: ", ")
        }
    }

    private var isScalar: Bool {
        switch self {
        case .array, .object: false
        default: true
        }
    }

    // JSON is a YAML input format and preserves literal map keys.
    var settingsYAML: String {
        let encoder = JSONEncoder()
        encoder.outputFormatting = [.sortedKeys, .withoutEscapingSlashes]
        return String(decoding: (try? encoder.encode(self)) ?? Data(), as: UTF8.self)
    }
}
