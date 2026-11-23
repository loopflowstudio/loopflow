import Loopflow
import SwiftUI

struct ScopedSettingRow: View {
    let model: ScopedSettingsModel
    let definition: ConfigurationDefinition
    let effective: ConfigurationValue
    let file: ConfigurationScope.File
    let source: String?
    let notices: NoticeCenter
    let manager: UndoManager?
    var highlightedKey: String?
    private struct Draft {
        var text: String
        let file: ConfigurationScope.File
        let observation: UUID
    }

    @State private var draft: Draft?
    @State private var reset: ConfigurationScope.File?
    @State private var patch: String?
    @State private var previewError: String?
    @State private var moveReview: MoveReview?
    @FocusState private var focused: Bool

    private var key: String { definition.key }
    private var slot: ConfigurationScope.Slot? { file.slots[key] }
    private var authored: JSONValue? { slot?.value }
    private var input: Binding<String> {
        Binding(get: { draft?.text ?? initialText }, set: { text in
            reset = nil
            patch = nil
            if draft == nil { draft = Draft(text: text, file: file, observation: model.observation) }
            else { draft?.text = text }
        })
    }
    private var initialText: String {
        guard let authored else { return "" }
        if key == "automation", case .object(let fields) = authored {
            return fields["retries"]?.settingsText ?? ""
        }
        return definition.controlType == "string" ? authored.settingsText : authored.settingsYAML
    }

    var body: some View {
        Section {
            control.settingsRow(key, highlighted: highlightedKey)
            DisclosureGroup {
                ForEach(Array(effective.candidates.enumerated()), id: \.offset) { _, candidate in
                    LabeledContent(candidate.locator) {
                        Text("\(candidate.key) · \(candidate.value.settingsText) · \(candidate.reason)")
                    }
                    .textSelection(.enabled)
                }
                ForEach(model.reading.value?.files.filter { $0.slots[key] != nil && $0.slots[key]?.value == nil } ?? []) { other in
                    LabeledContent(other.owner?.title ?? other.path, value: "No declaration")
                }
                Text(file.path).font(Typography.mono).textSelection(.enabled)
                if let reset {
                    Text(patch ?? previewError ?? "Preparing preview…").font(Typography.mono).textSelection(.enabled)
                    HStack {
                        if previewError != nil { Button("Refresh") { self.reset = nil; model.refresh() } }
                        Button("Cancel") { self.reset = nil }
                        Button("Reset in this file") { save(.reset, to: reset) }.disabled(patch == nil)
                    }
                } else if authored != nil {
                    Button("Reset layer…") { draft = nil; patch = nil; reset = file }
                }
                if let moveReview {
                    ForEach(moveReview.changes, id: \.path) { change in
                        Text(change.path).textSelection(.enabled)
                        Text(change.patch).font(Typography.mono).textSelection(.enabled)
                    }
                    LabeledContent("In effect after move", value: moveReview.effective.value.settingsText)
                    HStack {
                        Button("Cancel") { self.moveReview = nil }
                        Button("Confirm move") { promote(.confirm(moveReview)) }
                    }
                } else if case .object(let promotion) = slot?.promotion {
                    if promotion["kind"] == .string("resume") {
                        Button("Finish interrupted move…") { promote(.resume) }
                    } else if case .object(let address) = promotion["destination"] {
                        let title = address["kind"] == .string("shared_repo") ? "Move to Shared Repository…" : "Move to My Repository Settings…"
                        Button(title) { promote(.preview) }
                    }
                }
            } label: {
                LabeledContent(authored == nil ? "Inherited" : "In effect") {
                    Text([effective.value.settingsText, source].compactMap { $0 }.joined(separator: " · "))
                        .lineLimit(2)
                }
            }
        } footer: {
            Text(definition.description)
        }
        .task(id: PreviewInput(draft: draft?.text, resetting: reset != nil, observation: model.observation, entries: slot?.entries ?? [])) {
            patch = nil
            previewError = nil
            do {
                let result: String
                if let reset {
                    result = try await model.preview(.reset, key: key, file: reset)
                } else if let draft, usesYAML {
                    try await Task.sleep(for: .milliseconds(150))
                    result = try await model.preview(.set(draft.text), key: key, file: destination(for: draft))
                } else { return }
                if !Task.isCancelled { patch = result }
            } catch { if !Task.isCancelled { previewError = error.localizedDescription } }
        }
    }

    private struct PreviewInput: Equatable {
        let draft: String?
        let resetting: Bool
        let observation: UUID
        let entries: [ConfigurationEntry]
    }

    private var usesYAML: Bool {
        definition.controlType != "string" && definition.controlType != "integer" && key != "automation"
    }

    @ViewBuilder private var control: some View {
        if definition.controlType == "boolean" {
            ScopedBooleanToggle(model: model, definition: definition, effective: effective,
                                file: file, notices: notices, manager: manager)
        } else if case .array(let choices) = definition.controlSchema["enum"] {
            Picker(definition.title, selection: Binding(get: { authored ?? .null }, set: {
                save($0 == .null ? .reset : .set($0.settingsYAML), to: file)
            })) {
                Text("Inherited").tag(JSONValue.null)
                ForEach(choices, id: \.self) { Text($0.settingsText).tag($0) }
            }
        } else if definition.controlType == "array", case .object(let item) = definition.controlSchema["items"], item["type"] == .string("string") {
            OrderedSetting(title: definition.title, value: authored, save: { values in
                await model.apply(.set(JSONValue.array(values.map(JSONValue.string)).settingsYAML), key: key, to: file, notices: notices, manager: manager)
            })
        } else if !usesYAML {
            TextField(definition.title, text: input, prompt: Text("Inherited"))
                .focused($focused)
                .onSubmit { commitText() }
                .onChange(of: focused) { _, focused in if !focused { commitText() } }
                .onExitCommand { draft = nil }
        } else {
            DisclosureGroup(definition.title) {
                TextEditor(text: input).font(Typography.mono).frame(minHeight: 65)
                HStack {
                    Button("Cancel") { draft = nil }
                    Button("Apply") { commitText() }.disabled(patch == nil || previewError != nil)
                }.disabled(draft == nil)
                if let previewError {
                    Text(previewError).foregroundStyle(.secondary)
                    Button("Refresh") { model.refresh() }
                } else if let patch { Text(patch).font(Typography.mono).textSelection(.enabled) }
            }
        }
    }

    private func promote(_ action: MoveAction) {
        Task {
            moveReview = await model.promote(action, key: key, file: file,
                notices: notices, manager: manager)
        }
    }

    // Hand edits retain the draft’s expectation. Refresh explicitly takes a new reading.
    private func destination(for draft: Draft) -> ConfigurationScope.File {
        draft.observation == model.observation ? draft.file : file
    }

    private func commitText() {
        guard let draft, !model.saving else { return }
        let text: String
        if key == "automation" {
            // The file declares the automation mapping as one setting.
            let retries = Int64(draft.text).map(JSONValue.integer) ?? .string(draft.text)
            text = JSONValue.object(["retries": retries]).settingsYAML
        } else {
            text = definition.controlType == "string" ? JSONValue.string(draft.text).settingsYAML : draft.text
        }
        save(.set(text), to: destination(for: draft))
    }
    private func save(_ change: ConfigurationChange, to file: ConfigurationScope.File) {
        Task {
            if await model.apply(change, key: key, to: file, notices: notices, manager: manager) {
                draft = nil
                reset = nil
            }
        }
    }
}

struct ScopedBooleanToggle: View {
    let model: ScopedSettingsModel
    let definition: ConfigurationDefinition
    let effective: ConfigurationValue
    let file: ConfigurationScope.File
    let notices: NoticeCenter
    let manager: UndoManager?

    var body: some View {
        let key = definition.key
        Toggle(definition.title, isOn: Binding(
            get: { (file.slots[key]?.value ?? effective.value) == .bool(true) },
            set: { enabled in
                Task { await model.apply(.set(enabled ? "true" : "false"), key: key, to: file, notices: notices, manager: manager) }
            }
        ))
        .toggleStyle(.switch)
        .disabled(model.editsDisabled)
    }
}

extension ConfigurationDefinition {
    var controlSchema: [String: JSONValue] {
        guard case .object(let root) = schema else { return [:] }
        if case .array(let options) = root["anyOf"], case .object(let first) = options.first,
           case .string(let reference) = first["$ref"], case .object(let defs) = root["$defs"],
           case .object(let choice) = defs[String(reference.split(separator: "/").last ?? "")] { return choice }
        return root
    }
    var controlType: String? {
        switch controlSchema["type"] {
        case .string(let name): name
        case .array(let names): names.compactMap { if case .string(let name) = $0, name != "null" { name } else { nil } }.first
        default: nil
        }
    }
}

private struct OrderedSetting: View {
    let title: String
    let value: JSONValue?
    let save: ([String]) async -> Bool
    @State private var addition = ""

    private var items: [String] {
        guard case .array(let values) = value else { return [] }
        return values.map(\.settingsText)
    }

    var body: some View {
        DisclosureGroup(title) {
            ForEach(Array(items.enumerated()), id: \.offset) { index, item in
                HStack {
                    Text(item).textSelection(.enabled)
                    Spacer()
                    Button { move(index, by: -1) } label: { Image(systemName: "arrow.up") }
                        .disabled(index == 0).accessibilityLabel("Move \(item) up")
                    Button { move(index, by: 1) } label: { Image(systemName: "arrow.down") }
                        .disabled(index + 1 == items.count).accessibilityLabel("Move \(item) down")
                    Button {
                        var changed = items
                        changed.remove(at: index)
                        Task { _ = await save(changed) }
                    } label: { Image(systemName: "minus.circle") }.accessibilityLabel("Remove \(item)")
                }
            }
            HStack {
                TextField("New entry", text: $addition).onSubmit { add() }.onExitCommand { addition = "" }
                Button("Add") { add() }.disabled(addition.isEmpty)
            }
            if value != .array([]) { Button("Save empty list") { Task { _ = await save([]) } } }
        }
    }

    private func move(_ index: Int, by offset: Int) {
        var changed = items
        changed.swapAt(index, index + offset)
        Task { _ = await save(changed) }
    }

    private func add() {
        guard !addition.isEmpty else { return }
        let entry = addition
        Task { if await save(items + [entry]), addition == entry { addition = "" } }
    }
}
