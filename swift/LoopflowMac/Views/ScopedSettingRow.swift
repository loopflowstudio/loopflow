import Foundation
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
        // A nil text previews removing the declaration from this file.
        var text: String?
        var file: ConfigurationScope.File
    }

    @State private var draft: Draft?
    @State private var preview = Reading<String>.loading
    @State private var moveReview: MoveReview?
    @State private var updatedReview = false
    @FocusState private var focused: Bool

    private var key: String { definition.key }
    private var slot: ConfigurationScope.Slot? { file.slots[key] }
    private var declared: ConfigurationScope.Authored { slot?.declared ?? .absent }
    private var authored: JSONValue? { declared.value }
    private var editError: ScopedSettingsModel.RowError? { model.errors[.init(file, key)] }
    private var input: Binding<String> {
        Binding(get: { draft?.text ?? initialText }, set: { text in
            preview = .loading
            if draft?.text == nil { draft = Draft(text: text, file: file) }
            else { draft?.text = text }
        })
    }
    private var initialText: String {
        guard let value = definition.fieldValue(in: authored) else { return "" }
        if case .yaml = definition.control { return value.settingsYAML }
        return value == .null ? "" : value.settingsText
    }

    var body: some View {
        VStack(alignment: .leading, spacing: Spacing.xs) {
            control.help(definition.description)
            if let editError {
                Text(editError.message).foregroundStyle(.secondary).textSelection(.enabled)
                if editError.recovery == .refresh { Button("Refresh") { refreshRow() } }
            }
            if file.owner != .personalGlobal, definition.section != .policy,
               declared != .absent || effective.candidates.contains(where: { $0.source != "builtin" }) {
                SettingProvenance(title: "In effect",
                    value: [effectiveText, source ?? "Default"].joined(separator: " · "),
                    effective: effective, definition: definition, files: model.reading.value?.files ?? [])
            }
            if let draft, draft.text == nil {
                Text(preview.current ?? preview.failure ?? "Preparing preview…").font(Typography.mono).textSelection(.enabled)
                HStack {
                    Button("Cancel") { self.draft = nil }
                    Button("Reset in this file") { save(.reset, to: draft.file) }.disabled(preview.current == nil)
                    if preview.failure != nil { Button("Refresh") { refreshRow() } }
                }
            } else if declared != .absent {
                if case .yaml = definition.control {
                    Button("Reset…") {
                        preview = .loading
                        draft = Draft(text: nil, file: file)
                    }.controlSize(.small)
                } else {
                    Button("Reset") { save(.reset, to: file) }.controlSize(.small)
                }
            }
            moveControls
        }
        .disabled(model.editsDisabled)
        .task(id: PreviewInput(text: draft?.text, expected: draft?.file.slots[key]?.entries, entries: slot?.entries)) {
            preview = .loading
            guard let draft else { return }
            do {
                let change: ConfigurationChange
                if let text = draft.text {
                    guard case .yaml = definition.control else { return }
                    try await Task.sleep(for: .milliseconds(150))
                    change = .set(text)
                } else { change = .reset }
                let result = try await model.preview(change, key: key, file: draft.file)
                if !Task.isCancelled { preview = .available(result) }
            } catch {
                if !Task.isCancelled { preview = .unavailable(lastGood: nil, reason: error.localizedDescription) }
            }
        }
    }

    private struct PreviewInput: Equatable {
        let text: String?
        let expected: [ConfigurationEntry]?
        let entries: [ConfigurationEntry]?
    }

    @ViewBuilder private var moveControls: some View {
        if let moveReview {
            if updatedReview { Text("The move preview changed. Review it again before confirming.") }
            Text(file.owner == .personalWave ? "Applies to all work in this checkout." : "Applies to everyone using this repository.")
            ForEach(moveReview.changes, id: \.path) { change in
                Text(change.path).textSelection(.enabled)
                Text(change.patch).font(Typography.mono).textSelection(.enabled)
            }
            LabeledContent("In effect after move", value: definition.displayText(for: moveReview.effective.value))
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

    /// What applies while this file declares nothing, and where it comes from.
    /// The agent keys read their own declarations: their effective value is the combined agent order.
    var unsetText: String {
        if definition.section == .policy {
            guard let candidate = effective.candidates.last(where: { $0.key == key }) else {
                if let unset = definition.unset { return labeled(unset, origin: "Default") }
                let builtin = try? JSONDecoder().decode(JSONValue.self, from: Data(definition.default.utf8))
                return labeled(definition.displayText(for: builtin ?? .null), origin: "Default")
            }
            let origin = model.reading.value?.title(locator: candidate.locator) ?? candidate.locator
            return labeled(definition.displayText(for: candidate.value), origin: origin)
        }
        return labeled(effectiveText, origin: source ?? "Default")
    }

    /// A null default reads as what decides instead (`~/src`, Git's user.name), never "None".
    private var effectiveText: String {
        let value = definition.fieldValue(in: effective.value) ?? .null
        if value == .null, (source ?? "Default") == "Default", let unset = definition.unset { return unset }
        return definition.displayText(for: value)
    }

    private func labeled(_ text: String, origin: String) -> String {
        origin == "Default" ? "\(text) (Default)" : "\(text) (from \(origin))"
    }

    @ViewBuilder private var control: some View {
        switch definition.control {
        case .toggle:
            ScopedBooleanToggle(model: model, definition: definition, effective: effective,
                                file: file, notices: notices, manager: manager)
        case .choices(let choices):
            Picker(definition.title, selection: Binding(get: { declared }, set: { selection in
                switch selection {
                case .absent: save(.reset, to: file)
                case .present(let value): save(.set(value.settingsYAML), to: file)
                }
            })) {
                Text(unsetText).tag(ConfigurationScope.Authored.absent)
                if declared == .present(.null) {
                    Text("Explicit null").tag(ConfigurationScope.Authored.present(.null))
                }
                ForEach(choices, id: \.self) { Text(definition.displayText(for: $0)).tag(ConfigurationScope.Authored.present($0)) }
            }
        case .stringList:
            OrderedSetting(title: definition.title, value: authored, unsetText: unsetText,
                           canClearInherited: definition.combination == "append_reset" && file.owner != .personalGlobal, save: { values in
                await model.apply(.set(JSONValue.array(values.map(JSONValue.string)).settingsYAML), key: key, to: file, notices: notices, manager: manager)
            })
        case .text, .integer:
            TextField(definition.title, text: input, prompt: Text(declared == .present(.null) ? "Explicit null" : unsetText))
                .focused($focused)
                .onSubmit { commitText() }
                .onChange(of: focused) { _, focused in if !focused { commitText() } }
                .onExitCommand { draft = nil }
        case .yaml:
            DisclosureGroup(definition.title) {
                TextEditor(text: input).font(Typography.mono).frame(minHeight: 65)
                HStack {
                    Button("Cancel") { draft = nil }
                    Button("Apply") { commitText() }.disabled(preview.current == nil)
                }.disabled(draft?.text == nil)
                if let failure = preview.failure {
                    Text(failure).foregroundStyle(.secondary)
                    Button("Refresh") { refreshRow() }
                } else if let patch = preview.current { Text(patch).font(Typography.mono).textSelection(.enabled) }
            }
        }
    }

    private func promote(_ action: MoveAction) {
        Task {
            let review = await model.promote(action, key: key, file: file, notices: notices, manager: manager)
            moveReview = review
            if case .confirm = action { updatedReview = review != nil } else { updatedReview = false }
        }
    }

    private func refreshRow() {
        Task {
            guard let fresh = await model.refresh(key: key, file: file) else { return }
            draft?.file = fresh
        }
    }

    private func commitText() {
        guard let draft, let text = draft.text, !model.editsDisabled else { return }
        let change: ConfigurationChange
        switch definition.control {
        case .text:
            change = definition.change(to: text.isEmpty ? nil : .string(text), in: draft.file)
        case .integer:
            change = definition.integerChange(text, in: draft.file)
        default:
            change = .set(text)
        }
        save(change, to: draft.file)
    }

    private func save(_ change: ConfigurationChange, to file: ConfigurationScope.File) {
        Task {
            if await model.apply(change, key: key, to: file, notices: notices, manager: manager) {
                draft = nil
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
            get: { (definition.fieldValue(in: file.slots[key]?.declared.value) ?? definition.fieldValue(in: effective.value)) == .bool(true) },
            set: { enabled in
                let change = definition.change(to: .bool(enabled), in: file)
                Task { await model.apply(change, key: key, to: file, notices: notices, manager: manager) }
            }
        ))
        .toggleStyle(.switch)
        .disabled(model.editsDisabled)
    }
}

private struct OrderedSetting: View {
    let title: String
    let value: JSONValue?
    let unsetText: String
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
            LabeledContent(title, value: value?.settingsText ?? unsetText).lineLimit(1)
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
    var definition: ConfigurationDefinition?
    var files: [ConfigurationScope.File] = []

    var body: some View {
        DisclosureGroup {
            ForEach(Array(effective.candidates.enumerated()), id: \.offset) { _, candidate in
                let text = definition?.displayText(for: candidate.value) ?? candidate.value.settingsText
                LabeledContent(candidate.locator, value: "\(candidate.key) · \(text) · \(candidate.reason)")
                    .textSelection(.enabled)
            }
            if let key = definition?.key {
                ForEach(files.filter { $0.slots[key]?.declared == .absent }) { file in
                    LabeledContent(file.owner?.title ?? file.path, value: "No declaration")
                }
            }
        } label: {
            LabeledContent(title, value: value).lineLimit(2)
        }
    }
}
