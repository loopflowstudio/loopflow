import SwiftUI
import Foundation
import Loopflow

/// The catalogue supplies the available preferences; edits use the shared Rust file owner.
struct ConfigurationSettingsView: View {
    @State private var catalogue: [ConfigurationDefinition] = []
    @State private var destination = ConfigurationDestination.personalGlobal
    @State private var wave = ""
    @State private var key = "agent"
    @State private var value = ""
    @State private var repository = ""
    @State private var locator = ""
    @State private var explanation = ""
    @State private var notice = ""

    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            Text("Configuration").font(.title2)
            Picker("Save in", selection: $destination) {
                ForEach(ConfigurationDestination.allCases) { destination in
                    Text(destination.title).tag(destination)
                }
            }
            if destination != .personalGlobal {
                TextField("Project checkout", text: $repository)
            }
            if destination.isWave { TextField("Wave name or ID", text: $wave) }
            if !locator.isEmpty { Text(locator).font(.caption).textSelection(.enabled) }
            HStack {
                TextField("Setting key", text: $key)
                Menu("Browse") {
                    ForEach(catalogue.map(\.key), id: \.self) { name in
                        Button(name) { key = name }
                    }
                }
            }
            if let definition = catalogue.first(where: { $0.key == key.components(separatedBy: ".").first }) {
                Text(definition.description).font(.callout)
                Text("\(definition.applicability) · \(definition.combination) · \(definition.portability)")
                    .font(.caption).foregroundStyle(.secondary)
            }
            TextEditor(text: $value).font(.system(.body, design: .monospaced)).frame(minHeight: 100)
            HStack {
                Button("Reload", action: load)
                Button("Apply") { save(unset: false) }
                Button("Unset") { save(unset: true) }
            }
            Button("Explain effective value", action: inspect)
            if !notice.isEmpty { Text(notice).font(.callout).textSelection(.enabled) }
            ScrollView { Text(explanation).font(.caption).textSelection(.enabled).frame(maxWidth: .infinity, alignment: .leading) }
        }
        .padding(24).frame(minWidth: 620, minHeight: 550)
        .onChange(of: destination) { _, _ in resetSelection() }
        .onChange(of: repository) { _, _ in resetSelection() }
        .onChange(of: wave) { _, _ in resetSelection() }
        .onChange(of: key) { _, _ in resetSelection() }
        .onAppear {
            do { catalogue = try JSONDecoder().decode([ConfigurationDefinition].self, from: LoopflowLibrary.configuration(["action": "catalog"])); load() }
            catch { notice = error.localizedDescription }
        }
    }

    private var address: [String: Any] {
        destination.address(repository: (repository as NSString).expandingTildeInPath, wave: wave)
    }

    private func request(value: String?, unset: Bool) -> [String: Any] {
        ["action": "declaration", "destination": address, "key": key,
         "value": value as Any? ?? NSNull(), "unset": unset]
    }

    private func resetSelection() {
        value = ""
        locator = ""
        explanation = ""
        notice = ""
        load()
    }

    private func load() {
        do {
            let result = try JSONDecoder().decode(ConfigurationFileValue.self, from: LoopflowLibrary.configuration(request(value: nil, unset: false)))
            locator = result.path
            if result.value != .null {
                value = try configurationText(result.value)
                notice = "Loaded from \(result.path)"
            } else { value = ""; notice = "No declaration in this file. Unset reveals values from other sources." }
        } catch { notice = error.localizedDescription }
    }

    private func save(unset: Bool) {
        do {
            let result = try JSONDecoder().decode(ConfigurationFileValue.self, from: LoopflowLibrary.configuration(request(value: unset ? nil : value, unset: unset)))
            locator = result.path
            if unset { value = "" }
            notice = "\(unset ? "Removed declaration from" : "Saved to") \(result.path) · \(result.lifecycle.replacingOccurrences(of: "_", with: " "))"
            inspect()
        } catch { notice = error.localizedDescription }
    }

    private func inspect() {
        explanation = ""
        do {
            let result = try JSONDecoder().decode(ConfigurationExplanation.self, from: LoopflowLibrary.configuration(
                ["action": "explain", "destination": address, "key": key]))
            locator = result.path
            let effectiveKey = result.key
            let rootKey = effectiveKey.components(separatedBy: ".")[0]
            let settings = result.inspections.first?.settings ?? [:]
            var keys = settings.keys.filter { $0 == effectiveKey || $0.hasPrefix(effectiveKey + ".") }.sorted()
            if keys.isEmpty, settings[rootKey] != nil { keys = [rootKey] }
            guard !keys.isEmpty else { explanation = "No effective declaration for this key."; return }
            explanation = try keys.map { key in
                let selected = settings[key]!
                return "\(key): \(try configurationText(selected.value))\n" + (try selected.candidates.reversed().map {
                    "\($0.reason): \($0.source) · \($0.locator) · \($0.scope) · \(try configurationText($0.target)) = \(try configurationText($0.value))"
                }.joined(separator: "\n"))
            }.joined(separator: "\n\n")
        } catch { notice = error.localizedDescription }
    }
}

// This mirrors destination identity only; Rust derives paths and applicability.
enum ConfigurationDestination: String, CaseIterable, Identifiable {
    case personalGlobal = "personal_global"
    case personalRepository = "personal_repository"
    case sharedRepository = "shared_repository"
    case personalWave = "personal_wave"
    case sharedWave = "shared_wave"

    var id: String { rawValue }
    var isWave: Bool { self == .personalWave || self == .sharedWave }
    var title: String {
        switch self {
        case .personalGlobal: "My global preferences"
        case .personalRepository: "My project preferences"
        case .sharedRepository: "Shared project preferences"
        case .personalWave: "My Wave preferences"
        case .sharedWave: "Shared Wave preferences"
        }
    }

    func address(repository: String, wave: String) -> [String: Any] {
        var value: [String: Any] = ["kind": rawValue]
        if self != .personalGlobal { value["repository"] = repository }
        if isWave { value["wave"] = wave }
        return value
    }
}

struct ConfigurationExplanation: Decodable {
    let path: String
    let key: String
    let inspections: [ConfigurationInspection]
}

struct ConfigurationDefinition: Decodable {
    let key: String
    let schema: JSONValue
    let `default`: String
    let applicability: String
    let combination: String
    let portability: String
    let lifecycle: String
    let description: String
}

struct ConfigurationFileValue: Decodable {
    let path: String
    let key: String
    let value: JSONValue
    let lifecycle: String
}

struct ConfigurationInspection: Decodable {
    let skill: String?
    let occurrence: JSONValue?
    let settings: [String: ConfigurationValue]
}

struct ConfigurationValue: Decodable {
    let value: JSONValue
    let candidates: [ConfigurationCandidate]
}

struct ConfigurationCandidate: Decodable {
    let key: String
    let source: String
    let reason: String
    let locator: String
    let scope: String
    let target: JSONValue
    let value: JSONValue
}

private func configurationText(_ value: JSONValue) throws -> String {
    let encoder = JSONEncoder()
    encoder.outputFormatting = [.prettyPrinted, .sortedKeys]
    return String(decoding: try encoder.encode(value), as: UTF8.self)
}
