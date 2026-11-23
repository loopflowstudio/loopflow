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

struct ConfigurationExplanation: Decodable {
    let path: String
    let key: String
    let inspections: [ConfigurationInspection]
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

struct ConfigurationValue: Codable {
    let value: JSONValue
    let candidates: [ConfigurationCandidate]
}

struct ConfigurationCandidate: Codable {
    let key: String
    let source: String
    let reason: String
    let locator: String
    let scope: String
    let target: JSONValue
    let value: JSONValue
}

struct ConfigurationScope: Codable {
    let settings: [String: ConfigurationValue]
    let files: [File]

    struct File: Codable, Identifiable {
        let destination: JSONValue
        let path: String
        let slots: [String: Slot]
        var id: JSONValue { destination }

        var owner: ConfigurationDestination? {
            guard case .object(let fields) = destination, case .string(let kind) = fields["kind"] else { return nil }
            return ConfigurationDestination(rawValue: kind)
        }
    }

    struct Slot: Codable {
        let key: String
        let declared: JSONValue
        let entries: [ConfigurationEntry]
        let promotion: JSONValue

        var value: JSONValue? {
            guard case .object(let fields) = declared, fields["kind"] == .string("present") else { return nil }
            return fields["value"]
        }
    }

    func source(_ key: String) -> String? {
        let locators = Set(settings[key]?.candidates.filter { $0.reason == "selected" }.map(\.locator) ?? [])
        guard locators.count == 1, let path = locators.first else { return nil }
        return files.first { file in
            if file.owner == .personalWave { return path.hasPrefix(file.path + "#waves.") }
            return file.path == path
        }?.owner?.title ?? path
    }
}

struct ConfigurationEditReceipt: Codable {
    let before: [ConfigurationEntry]
    let after: [ConfigurationEntry]
}

extension JSONValue {
    var settingsText: String {
        switch self {
        case .null: return "None"
        case .string(let text): return text
        case .bool(let enabled): return enabled ? "On" : "Off"
        default: return settingsYAML
        }
    }

    // JSON is a YAML input format and preserves literal map keys.
    var settingsYAML: String {
        let encoder = JSONEncoder()
        encoder.outputFormatting = [.sortedKeys, .withoutEscapingSlashes]
        return String(decoding: (try? encoder.encode(self)) ?? Data(), as: UTF8.self)
    }
}

struct MoveReview: Codable {
    struct Patch: Codable { let path: String; let patch: String }
    let changes: [Patch]
    let effective: ConfigurationValue
    let receipt: MoveReceipt
}

struct MoveReceipt: Codable {
    let source: ConfigurationEditReceipt
    let receiving: ConfigurationEditReceipt
}

enum MoveResult: Decodable {
    case preview(MoveReview)
    case applied(MoveReceipt)
    case pending
    case recovered

    private enum Keys: String, CodingKey { case kind, review, receipt }
    private enum Kind: String, Decodable { case preview, applied, pending, recovered }
    init(from decoder: Decoder) throws {
        let fields = try decoder.container(keyedBy: Keys.self)
        switch try fields.decode(Kind.self, forKey: .kind) {
        case .preview: self = .preview(try fields.decode(MoveReview.self, forKey: .review))
        case .applied: self = .applied(try fields.decode(MoveReceipt.self, forKey: .receipt))
        case .pending: self = .pending
        case .recovered: self = .recovered
        }
    }
}

extension JSONValue {
    init(settingsValue: some Encodable) throws {
        self = try JSONDecoder().decode(Self.self, from: JSONEncoder().encode(settingsValue))
    }
}
