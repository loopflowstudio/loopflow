import AppKit
import SwiftUI
import Loopflow

struct ThemeChooser: View {
    let appearance: AppearanceStore
    @State private var editing = false
    @Environment(\.dismiss) private var dismiss
    @Environment(\.palette) private var palette

    var body: some View {
        let terminal = appearance.selectedTheme.terminal
        VStack(alignment: .leading, spacing: 16) {
            Text("Choose Theme").font(Typography.sectionTitle())
            HStack(alignment: .top, spacing: 16) {
                VStack(alignment: .leading, spacing: 6) {
                    Text("Mode").font(.caption).foregroundStyle(palette.textSecondary)
                    Picker("Mode", selection: Binding(get: { appearance.current.mode }, set: { appearance.previewMode($0) })) {
                        ForEach(AppearanceMode.allCases, id: \.self) { mode in Text(mode.menuTitle).tag(mode) }
                    }
                    .pickerStyle(.segmented)
                    .labelsHidden()
                    .frame(height: 36)
                    .accessibilityIdentifier("appearance-mode")
                }
                .frame(width: 180)
                ForEach([AppearanceMode.light, .dark], id: \.self) { slot in
                    let enabled = appearance.current.mode == .system || appearance.current.mode == slot
                    VStack(alignment: .leading, spacing: 6) {
                        Text("\(slot.menuTitle) theme").font(.caption).foregroundStyle(palette.textSecondary)
                        Button { appearance.editSlot(slot) } label: {
                            Text(appearance.themeName(for: slot))
                                .lineLimit(1).truncationMode(.tail)
                                .frame(maxWidth: .infinity, minHeight: 28, alignment: .leading)
                        }
                        .buttonStyle(WorkOutlineButtonStyle())
                        .workSelection(enabled && appearance.editingSlot == slot)
                        .disabled(!enabled)
                        .accessibilityLabel("\(slot.menuTitle) theme")
                        .accessibilityValue(appearance.themeName(for: slot))
                        .accessibilityIdentifier("appearance-slot-\(slot.rawValue)")
                        .help(enabled ? "Choose your \(slot.menuTitle.lowercased()) theme" : "Used in \(slot.menuTitle) or System mode")
                    }
                    .frame(maxWidth: .infinity, alignment: .leading)
                    .opacity(enabled ? 1 : 0.45)
                }
            }
            Text("\(ThemeDefinition.slotCount) themes · Choose your \(appearance.editingSlot.menuTitle.lowercased()) theme · previewing \(appearance.selectedTheme.name)")
                .font(.caption).foregroundStyle(palette.textSecondary)
            ScrollView {
                if editing {
                    ThemeEditor(appearance: appearance)
                } else {
                LazyVGrid(columns: [GridItem(.adaptive(minimum: 150))], spacing: 12) {
                    ForEach(appearance.themes) { theme in
                        Button { appearance.previewTheme(theme.id) } label: {
                            VStack(alignment: .leading, spacing: 8) {
                                Text(theme.name).foregroundStyle(theme.palette.text)
                                HStack(spacing: 3) {
                                    ForEach(0..<8) { i in Rectangle().fill(theme.terminal.indexed[i].color).frame(height: 8) }
                                }
                                Text("❯ Work stays here").font(.system(size: 10, design: .monospaced))
                                    .foregroundStyle(theme.terminal.foreground.color)
                                    .padding(5).background(theme.terminal.background.color)
                            }
                            .padding(10).frame(maxWidth: .infinity)
                            .background(theme.palette.background)
                        }
                        .buttonStyle(.plain)
                        .workSelection(appearance.selectedID == theme.id)
                        .workFocusRing()
                        .accessibilityIdentifier("appearance-theme-\(theme.id)")
                    }
                }
                }
                HStack(spacing: 12) {
                    Text("Selected Work").foregroundStyle(palette.workSelectedText)
                        .padding(6).workDestinationSelection(true)
                    Text("Working").foregroundStyle(palette.working)
                    Text("Waiting").foregroundStyle(palette.waiting)
                    Text("Error").foregroundStyle(palette.failure)
                }.padding(.vertical, 14)
                VStack(alignment: .leading, spacing: 6) {
                    ForEach(appearance.diagnostics, id: \.self) { Text($0) }
                    ForEach(appearance.libraryErrors, id: \.self) { Text($0).foregroundStyle(palette.failure) }
                    DisclosureGroup("Terminal contrast details") {
                        ForEach(terminal.ansiWarnings, id: \.self) { Text($0).foregroundStyle(palette.textSecondary) }
                        ForEach(terminal.dimWarnings, id: \.self) { Text($0).foregroundStyle(palette.textSecondary) }
                    }
                    Text("Explicit colors printed by terminal applications may retain their own appearance.")
                        .foregroundStyle(palette.textSecondary)
                }.font(.caption).frame(maxWidth: .infinity, alignment: .leading)
            }
            if let notice = appearance.notice { Text(notice).foregroundStyle(palette.failure).textSelection(.enabled) }
            HStack {
                Button(editing ? "Done Editing" : "Edit") { editing.toggle() }
                    .accessibilityIdentifier("appearance-edit-theme")
                Button("Reset") { appearance.resetTheme() }
                    .disabled(!appearance.canReset)
                    .help("Restore \(appearance.originalTheme.name), including its name")
                    .accessibilityIdentifier("appearance-reset-theme")
                Button("Import…", action: importTheme)
                    .help("Replace this slot with a Ghostty or complete Loopflow theme")
                    .accessibilityIdentifier("appearance-import-theme")
                Spacer()
                Button("Cancel") { appearance.cancel() }.keyboardShortcut(.cancelAction)
                Button("Apply") { appearance.apply() }.keyboardShortcut(.defaultAction)
                    .disabled(!appearance.canApply)
            }.buttonStyle(WorkOutlineButtonStyle())
        }
        .padding(20).frame(minWidth: 620, minHeight: 560)
        .foregroundStyle(palette.text).background(palette.background)
        .onAppear { appearance.beginPreview() }
        .onChange(of: appearance.chooserOpen) { _, open in if !open { dismiss() } }
        .onDisappear { if appearance.chooserOpen { appearance.cancel() } }
    }

    private func importTheme() {
        let panel = NSOpenPanel()
        panel.canChooseDirectories = false
        panel.allowsMultipleSelection = false
        panel.prompt = "Replace Theme"
        let slotID = appearance.selectedID
        panel.begin { response in
            guard response == .OK, let url = panel.url else { return }
            appearance.importTheme(url, replacing: slotID)
        }
    }
}
