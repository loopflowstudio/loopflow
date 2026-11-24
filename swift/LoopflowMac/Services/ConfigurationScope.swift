import Foundation
import Loopflow

// Rust derives paths and applicability from the destination.
enum ConfigurationDestination: String, Identifiable {
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
    /// What an unset value means when the default is null and something outside the files decides.
    let unset: String?

    private enum Keys: String, CodingKey {
        case key, shorthands, category, schema, `default`, applicability, combination, portability, lifecycle, description, unset
    }

    init(from decoder: Decoder) throws {
        let fields = try decoder.container(keyedBy: Keys.self)
        key = try fields.decode(String.self, forKey: .key)
        shorthands = try fields.decode([String].self, forKey: .shorthands)
        category = try fields.decode(String.self, forKey: .category)
        schema = try fields.decode(JSONValue.self, forKey: .schema)
        `default` = try fields.decode(String.self, forKey: .default)
        applicability = try fields.decode(String.self, forKey: .applicability)
        combination = try fields.decode(String.self, forKey: .combination)
        portability = try fields.decode(String.self, forKey: .portability)
        lifecycle = try fields.decode(String.self, forKey: .lifecycle)
        description = try fields.decode(String.self, forKey: .description)
        unset = try fields.decode(String?.self, forKey: .unset)
    }

    enum Control {
        case toggle, text, integer, stringList, yaml
        case choices([JSONValue])
    }

    // These controls edit one field of a mapping. Checked writes still cover
    // the whole declaration, including its other fields.
    var editedField: String? {
        switch key {
        case "user": "name"
        case "logs": "linear_requests"
        case "automation": "retries"
        default: nil
        }
    }

    func fieldValue(in value: JSONValue?) -> JSONValue? {
        if let editedField { return value?[editedField] }
        return value
    }

    func change(to value: JSONValue?, in declaration: SettingDeclaration) -> ConfigurationChange {
        guard let editedField else { return value.map { .set($0.settingsYAML) } ?? .reset }
        var fields: [String: JSONValue] = [:]
        if case .present(.object(let existing)) = declaration.slot.declared { fields = existing }
        fields[editedField] = value
        return fields.isEmpty ? .reset : .set(JSONValue.object(fields).settingsYAML)
    }

    func integerChange(_ text: String, in declaration: SettingDeclaration) -> ConfigurationChange {
        let input = text.trimmingCharacters(in: .whitespacesAndNewlines)
        if input.isEmpty { return change(to: nil, in: declaration) }
        guard editedField != nil else { return .set(input) }
        return change(to: Int64(input).map(JSONValue.integer) ?? .string(input), in: declaration)
    }

    func displayText(for value: JSONValue) -> String {
        switch (key, value) {
        case ("land", .string("gh")): "GitHub PR"
        case ("land", .string("local")): "Local merge"
        case ("codex.service_tier", .string("default")): "Standard"
        case ("codex.service_tier", .string("priority")): "Fast"
        case ("auto", .string("on")): "On"
        case ("auto", .string("off")): "Off"
        case ("merge_method", .string(let method)): method.capitalized
        default: value.settingsText
        }
    }

    var control: Control {
        var field = editedField.map { schema["properties"][$0] } ?? schema
        var nullable = false
        if case .array(let options) = field["anyOf"] {
            nullable = options.contains { $0["type"] == .string("null") }
            if case .string(let ref) = options.first?["$ref"], let name = ref.split(separator: "/").last {
                field = schema["$defs"][String(name)]
            }
        }
        var type = field["type"]
        if case .array(let types) = type {
            nullable = types.contains(.string("null"))
            type = types.first { $0 != .string("null") } ?? .null
        }
        if case .array(let values) = field["enum"] { return .choices(values) }
        switch type {
        case .string("boolean"): return nullable ? .choices([.bool(true), .bool(false)]) : .toggle
        case .string("string"): return .text
        case .string("integer"): return .integer
        case .string("array") where field["items"]["type"] == .string("string"): return .stringList
        default: return .yaml
        }
    }
}

struct ConfigurationInspection: Decodable {
    let skill: String?
    let occurrence: JSONValue?
    let settings: [String: ConfigurationValue]
}

/// A checked edit carries the addressed slot as it was when editing began.
struct SettingDeclaration: Identifiable {
    struct ID: Hashable {
        let destination: JSONValue
        let key: String
    }

    let catalogKey: String
    let destination: JSONValue
    let path: String
    let owner: ConfigurationDestination?
    let slot: ConfigurationScope.Slot

    var id: ID { ID(destination: destination, key: slot.key) }
}

struct ConfigurationScope: Codable {
    let settings: [String: ConfigurationValue]
    let files: [File]

    struct File: Codable, Identifiable {
        let destination: JSONValue
        let path: String
        let readPath: String
        let slots: [String: Slot]
        var id: JSONValue { destination }

        private enum CodingKeys: String, CodingKey {
            case destination, path, slots
            case readPath = "read_path"
        }

        func declaration(_ key: String) -> SettingDeclaration? {
            guard let slot = slots[key] else { return nil }
            return SettingDeclaration(catalogKey: key, destination: destination, path: path, owner: owner, slot: slot)
        }

        var hasOverrides: Bool {
            owner != .personalGlobal && slots.values.contains { !$0.entries.isEmpty }
        }

        func prepareForOpening() throws -> URL {
            let url = URL(fileURLWithPath: readPath)
            if !FileManager.default.fileExists(atPath: readPath) {
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

    func current(_ declaration: SettingDeclaration) -> SettingDeclaration? {
        let file = files.first { $0.destination == declaration.destination }
        guard let current = file?.declaration(declaration.catalogKey), current.id == declaration.id else { return nil }
        return current
    }

    func source(_ key: String) -> String? {
        guard let candidates = settings[key]?.candidates else { return nil }
        var sources = candidates.filter { $0.reason == "selected" }
        if sources.isEmpty {
            // Append/reset lists can have several sources and no single winner.
            sources = candidates.filter {
                $0.reason == "contributes" || ($0.reason == "clears earlier contributions" && $0.source != "builtin")
            }
        }
        let titles = Set(sources.map { title(locator: $0.locator) }).sorted()
        return titles.isEmpty ? nil : titles.joined(separator: ", ")
    }

    func title(locator path: String) -> String {
        if path == "builtin" { return "Default" }
        return files.first { file in
            if file.owner == .personalWave { return path.hasPrefix(file.path + "#waves.") }
            return file.readPath == path
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
