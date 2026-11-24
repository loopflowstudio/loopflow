import Loopflow
import SwiftUI

struct SettingsTargetPicker: View {
    let model: ScopedSettingsModel
    let checkout: String
    private struct Entry: Decodable { let name: String; let kind: String }

    // config.settings includes every effective setting per step. Only decode the menu choice.
    struct Step: Decodable {
        let choice: (title: String, target: ConfigurationTarget)?

        private enum Keys: String, CodingKey { case skill, occurrence }
        private enum Occurrence: String, CodingKey { case graph, id, address }

        init(from decoder: Decoder) throws {
            let fields = try decoder.container(keyedBy: Keys.self)
            let skill = try fields.decode(String?.self, forKey: .skill)
            guard try !fields.decodeNil(forKey: .occurrence) else { choice = nil; return }
            let occurrence = try fields.nestedContainer(keyedBy: Occurrence.self, forKey: .occurrence)
            let graph = try occurrence.decode(String.self, forKey: .graph)
            let id = try occurrence.decode(String?.self, forKey: .id)
            let address = try occurrence.decode(String?.self, forKey: .address)
            choice = (id ?? address).map { step in
                ("\(skill ?? step) · \(graph) / \(step)", .step(graph: graph, step: step))
            }
        }
    }

    @State private var expanded = false
    @State private var entries: [Entry] = []
    @State private var graph: String?
    @State private var steps: [Step] = []
    @State private var error: String?

    var body: some View {
        DisclosureGroup("Skill or Graph step", isExpanded: $expanded) {
            Menu("Inspect Skill") {
                ForEach(entries.filter { $0.kind == "skill" }, id: \.name) { entry in
                    Button(entry.name) { model.work.target = .skill(entry.name) }
                }
            }
            Picker("Graph", selection: $graph) {
                Text("Choose Graph").tag(String?.none)
                ForEach(entries.filter { $0.kind == "graph" }, id: \.name) { entry in
                    Text(entry.name).tag(Optional(entry.name))
                }
            }
            if graph != nil {
                Menu("Inspect Graph step") {
                    ForEach(Array(steps.enumerated()), id: \.offset) { _, inspection in
                        if let choice = inspection.choice {
                            Button(choice.title) { model.work.target = choice.target }
                        }
                    }
                }
            }
            if let error { Text(error) }
        }
        .task(id: expanded ? checkout : nil) {
            guard expanded else { return }
            do {
                let entries = try await ServerClient.read(.skillList(directory: checkout, path: []), location: model.location).decode([Entry].self)
                if Task.isCancelled { return }
                self.entries = entries
                error = nil
            } catch { if !Task.isCancelled { self.error = error.localizedDescription } }
        }
        .task(id: graph) {
            steps = []
            guard let graph else { return }
            do {
                let value = try await ServerClient.read(.configSettings(directory: checkout, wave: model.work.wave, skill: nil,
                    graph: graph, step: nil), location: model.location).decode([Step].self)
                if !Task.isCancelled { steps = value; error = nil }
            } catch { if !Task.isCancelled { self.error = error.localizedDescription } }
        }
    }
}
