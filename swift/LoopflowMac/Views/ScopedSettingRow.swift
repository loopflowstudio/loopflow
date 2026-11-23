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
    private struct Draft {
        var text: String
        var file: ConfigurationScope.File
    }

    @State private var draft: Draft?
    @State private var reset: ConfigurationScope.File?
    @State private var patch: String?
    @State private var previewError: String?
    @State private var moveReview: MoveReview?
    @State private var updatedReview = false
    @FocusState private var focused: Bool

    private var key: String { definition.key }
    private var slot: ConfigurationScope.Slot? { file.slots[key] }
    private var address: ScopedSettingsModel.RowAddress { .init(file, key) }
    private var isCurrentRow: Bool {
        model.file(for: definition).map { ScopedSettingsModel.RowAddress($0, key) == address } == true
    }
    private var declared: ConfigurationScope.Authored { slot?.declared ?? .absent }
    private var authored: JSONValue? { declared.value }
    private var editError: ScopedSettingsModel.RowError? { model.errors[address] }
    private var field: String? {
        switch key {
        case "user": "name"
        case "logs": "linear_requests"
        case "automation": "retries"
        default: nil
        }
    }
    private var input: Binding<String> {
        Binding(get: { draft?.text ?? initialText }, set: { text in
            reset = nil
            patch = nil
            if draft == nil { draft = Draft(text: text, file: file) }
            else { draft?.text = text }
        })
    }
    private var initialText: String {
        let value = field.flatMap { authored?[$0] } ?? authored
        guard let value else { return "" }
        if usesYAML { return value.settingsYAML }
        return value == .null ? "" : value.settingsText
    }

    var body: some View {
        VStack(alignment: .leading, spacing: Spacing.xs) {
            control.help(definition.description)
            if let editError {
                Text(editError.message).foregroundStyle(.secondary).textSelection(.enabled)
                if editError.recovery == .refresh { Button("Refresh") { refreshRow() } }
            }
            if file.owner != .personalGlobal && definition.section != .policy {
                SettingProvenance(title: declared == .absent ? "Inherited" : "In effect",
                    value: [effective.value.settingsText, source].compactMap { $0 }.joined(separator: " · "),
                    effective: effective, files: model.reading.value?.files ?? [], key: key, path: file.path)
            } else if file.owner == .personalGlobal && definition.pane != .general && definition.section != .policy {
                SettingProvenance(title: "Source", value: "", effective: effective)
            }
            if let reset {
                Text(patch ?? previewError ?? "Preparing preview…").font(Typography.mono).textSelection(.enabled)
                HStack {
                    Button("Cancel") { self.reset = nil }
                    Button("Reset in this file") { save(.reset, to: reset) }.disabled(patch == nil)
                    if previewError != nil { Button("Refresh") { refreshRow() } }
                }
            } else if declared != .absent {
                Button("Reset layer…") { draft = nil; patch = nil; reset = file }
                    .controlSize(.small)
            }
            moveControls
        }
        .disabled(model.editsDisabled)
        .onChange(of: address) {
            draft = nil
            reset = nil
            patch = nil
            previewError = nil
            moveReview = nil
            updatedReview = false
        }
        .task(id: PreviewInput(draft: draft?.text, resetting: reset != nil, expected: (reset ?? draft?.file)?.slots[key]?.entries, entries: slot?.entries ?? [])) {
            patch = nil
            previewError = nil
            do {
                let result: String
                if let reset {
                    result = try await model.preview(.reset, key: key, file: reset)
                } else if let draft, usesYAML {
                    try await Task.sleep(for: .milliseconds(150))
                    result = try await model.preview(.set(draft.text), key: key, file: draft.file)
                } else { return }
                if !Task.isCancelled { patch = result }
            } catch { if !Task.isCancelled { previewError = error.localizedDescription } }
        }
    }

    private struct PreviewInput: Equatable {
        let draft: String?
        let resetting: Bool
        let expected: [ConfigurationEntry]?
        let entries: [ConfigurationEntry]
    }

    @ViewBuilder private var moveControls: some View {
        if let moveReview {
            if updatedReview { Text("The move preview changed. Review it again before confirming.") }
            Text(file.owner == .personalWave ? "Applies to all work in this checkout." : "Applies to everyone using this repository.")
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
                Text("A move between files was interrupted.")
                Button("Finish interrupted move…") { promote(.resume) }
                if editError?.recovery == .dismissMove {
                    Text("Dismiss keeps both files as they are.")
                    Button("Dismiss pending move") { promote(.dismiss) }
                }
            } else if case .object(let destination) = promotion["destination"] {
                Button(destination["kind"] == .string("shared_repo") ? "Move to Shared Repository…" : "Move to My Repository Settings…") {
                    promote(.preview)
                }
            }
        }
    }

    private var usesYAML: Bool {
        definition.controlType != "string" && definition.controlType != "integer" && field == nil
    }

    @ViewBuilder private var control: some View {
        if key == "logs" {
            Toggle(definition.title, isOn: Binding(get: {
                (authored?["linear_requests"] ?? effective.value["linear_requests"]) == .bool(true)
            }, set: { save(fieldChange(.bool($0), in: file), to: file) }))
            .toggleStyle(.switch)
        } else if definition.controlType == "boolean" && !definition.nullable {
            ScopedBooleanToggle(model: model, definition: definition, effective: effective,
                                file: file, notices: notices, manager: manager)
        } else if let choices = definition.choices {
            Picker(definition.title, selection: Binding(get: { declared }, set: { selection in
                switch selection {
                case .absent: save(.reset, to: file)
                case .present(let value): save(.set(value.settingsYAML), to: file)
                }
            })) {
                Text("Inherited").tag(ConfigurationScope.Authored.absent)
                if definition.nullable {
                    Text("Explicit null").tag(ConfigurationScope.Authored.present(.null))
                }
                ForEach(choices, id: \.self) { Text($0.settingsText).tag(ConfigurationScope.Authored.present($0)) }
            }
        } else if definition.controlType == "array", case .object(let item) = definition.controlSchema["items"], item["type"] == .string("string") {
            OrderedSetting(title: definition.title, value: authored,
                           canClearInherited: definition.combination == "append_reset" && file.owner != .personalGlobal, save: { values in
                await model.apply(.set(JSONValue.array(values.map(JSONValue.string)).settingsYAML), key: key, to: file, notices: notices, manager: manager)
            })
            .id(address)
        } else if !usesYAML {
            TextField(definition.title, text: input, prompt: Text(declared == .present(.null) ? "Cleared (null)" : "Inherited"))
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
                    Button("Refresh") { refreshRow() }
                } else if let patch { Text(patch).font(Typography.mono).textSelection(.enabled) }
            }
        }
    }

    private func promote(_ action: MoveAction) {
        Task {
            let review = await model.promote(action, key: key, file: file, notices: notices, manager: manager)
            guard isCurrentRow else { return }
            moveReview = review
            if case .confirm = action { updatedReview = review != nil } else { updatedReview = false }
        }
    }

    private func refreshRow() {
        Task {
            guard let fresh = await model.refresh(key: key, file: file), isCurrentRow else { return }
            draft?.file = fresh
            if reset != nil { reset = fresh }
        }
    }

    private func fieldChange(_ value: JSONValue?, in file: ConfigurationScope.File) -> ConfigurationChange {
        guard let field else { return value.map { .set($0.settingsYAML) } ?? .reset }
        var fields: [String: JSONValue] = [:]
        if case .present(.object(let existing)) = file.slots[key]?.declared { fields = existing }
        fields[field] = value
        return fields.isEmpty ? .reset : .set(JSONValue.object(fields).settingsYAML)
    }

    private func commitText() {
        guard let draft, !model.editsDisabled else { return }
        let change: ConfigurationChange
        if key == "automation" {
            change = fieldChange(Int64(draft.text).map(JSONValue.integer) ?? .string(draft.text), in: draft.file)
        } else if definition.controlType == "string" || key == "user" {
            change = fieldChange(draft.text.isEmpty ? nil : .string(draft.text), in: draft.file)
        } else { change = .set(draft.text) }
        save(change, to: draft.file)
    }

    private func save(_ change: ConfigurationChange, to file: ConfigurationScope.File) {
        Task {
            if await model.apply(change, key: key, to: file, notices: notices, manager: manager), isCurrentRow {
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
            get: { (file.slots[key]?.declared.value ?? effective.value) == .bool(true) },
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
    var nullable: Bool {
        if case .array(let types) = schema["type"] { return types.contains(.string("null")) }
        if case .array(let options) = schema["anyOf"] { return options.contains { $0["type"] == .string("null") } }
        return false
    }

    var choices: [JSONValue]? {
        if controlType == "boolean" && nullable { return [.bool(true), .bool(false)] }
        if case .array(let values) = controlSchema["enum"] { return values }
        return nil
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
    let canClearInherited: Bool
    let save: ([String]) async -> Bool
    @State private var addition = ""

    private var items: [String] {
        guard case .array(let values) = value else { return [] }
        return values.map(\.settingsText)
    }

    var body: some View {
        DisclosureGroup {
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
            if canClearInherited && value != .array([]) { Button("Save empty list") { Task { _ = await save([]) } } }
        } label: {
            LabeledContent(title, value: value?.settingsText ?? "Inherited").lineLimit(1)
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

struct SettingProvenance: View {
    let title: String
    let value: String
    let effective: ConfigurationValue
    var files: [ConfigurationScope.File] = []
    var key: String?
    var path: String?

    var body: some View {
        DisclosureGroup {
            ForEach(Array(effective.candidates.enumerated()), id: \.offset) { _, candidate in
                LabeledContent(candidate.locator, value: "\(candidate.key) · \(candidate.value.settingsText) · \(candidate.reason)")
                    .textSelection(.enabled)
            }
            if let key {
                ForEach(files.filter { $0.slots[key]?.declared == .absent }) { file in
                    LabeledContent(file.owner?.title ?? file.path, value: "No declaration")
                }
            }
            if let path { Text(path).font(Typography.mono).textSelection(.enabled) }
        } label: {
            LabeledContent(title, value: value).lineLimit(2)
        }
    }
}
