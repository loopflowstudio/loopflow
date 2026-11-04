import SwiftUI
import Foundation
import Loopflow

/// The catalogue supplies the available preferences; edits use the shared Rust file owner.
struct ConfigurationSettingsView: View {
    @State private var catalogue: [ConfigurationDefinition] = []
    @State private var file = ThemePaths.standard.config.path
    @State private var scope = "global"
    @State private var shared = false
    @State private var wave = ""
    @State private var key = "agent"
    @State private var value = ""
    @State private var repository = NSHomeDirectory()
    @State private var explanation = ""
    @State private var notice = ""

    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            Text("Configuration").font(.title2)
            TextField("Configuration file", text: $file)
            HStack {
                Picker("Scope", selection: $scope) {
                    Text("Global").tag("global")
                    Text("Repository").tag("repository")
                    Text("Wave").tag("wave")
                }
                Toggle("Shared configuration", isOn: $shared)
            }
            if scope == "wave" { TextField("Wave name", text: $wave) }
            HStack {
                TextField("Setting key", text: $key)
                Menu("Browse") {
                    ForEach(catalogue.map(\.key), id: \.self) { name in
                        Button(name) { key = name; load() }
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
            TextField("Repository to inspect", text: $repository)
            Button("Explain effective value") { Task { await inspect() } }
            if !notice.isEmpty { Text(notice).font(.callout).textSelection(.enabled) }
            ScrollView { Text(explanation).font(.caption).textSelection(.enabled).frame(maxWidth: .infinity, alignment: .leading) }
        }
        .padding(24).frame(minWidth: 620, minHeight: 550)
        .onAppear {
            do { catalogue = try JSONDecoder().decode([ConfigurationDefinition].self, from: LoopflowLibrary.configuration(["action": "catalog"])); load() }
            catch { notice = error.localizedDescription }
        }
    }

    private func request(value: String?, unset: Bool) -> [String: Any] {
        ["action": "file", "path": (file as NSString).expandingTildeInPath, "key": scope == "wave" && !shared ? "waves.\(wave).\(key)" : key,
         "value": value as Any? ?? NSNull(), "unset": unset, "scope": scope, "shared": shared]
    }

    private func load() {
        do {
            let result = try JSONDecoder().decode(ConfigurationFileValue.self, from: LoopflowLibrary.configuration(request(value: nil, unset: false)))
            if result.value != .null {
                value = try configurationText(result.value)
                notice = "Loaded from \(file)"
            } else { value = ""; notice = "No declaration in this file. Unset reveals values from other sources." }
        } catch { notice = error.localizedDescription }
    }

    private func save(unset: Bool) {
        do {
            let result = try JSONDecoder().decode(ConfigurationFileValue.self, from: LoopflowLibrary.configuration(request(value: unset ? nil : value, unset: unset)))
            notice = "\(unset ? "Removed declaration from" : "Saved to") \(result.path) · \(result.lifecycle.replacingOccurrences(of: "_", with: " "))"
        } catch { notice = error.localizedDescription }
    }

    private func inspect() async {
        let parts = key.components(separatedBy: ".")
        let skill = parts.first == "skills" && parts.count > 2 ? parts[1] : nil
        let graph = parts.first == "graphs" && parts.count > 4 ? parts[1] : nil
        let step = graph == nil ? nil : parts[3]
        let effectiveKey = skill != nil ? parts.dropFirst(2).joined(separator: ".") : graph != nil ? parts.dropFirst(4).joined(separator: ".") : key
        do {
            let text = try await LoopflowLibrary.read(.settings(directory: (repository as NSString).expandingTildeInPath, wave: wave.isEmpty ? nil : wave, skill: skill, graph: graph, step: step))
            let rows = try JSONDecoder().decode([ConfigurationInspection].self, from: Data(text.utf8))
            let rootKey = effectiveKey.components(separatedBy: ".")[0]
            let settings = rows.first?.settings ?? [:]
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
