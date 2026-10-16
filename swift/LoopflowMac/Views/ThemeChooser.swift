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
        let entries = appearance.entries
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
                ForEach([AppearanceMode.light, .dark], id: \.self) { side in
                    let enabled = appearance.current.mode == .system || appearance.current.mode == side
                    VStack(alignment: .leading, spacing: 6) {
                        Text("\(side.menuTitle) theme").font(.caption).foregroundStyle(palette.textSecondary)
                        Button { editing = false; appearance.showPicker(side) } label: {
                            Text(appearance.themeName(for: side))
                                .lineLimit(1).truncationMode(.tail)
                                .frame(maxWidth: .infinity, minHeight: 28, alignment: .leading)
                        }
                        .buttonStyle(WorkOutlineButtonStyle())
                        .workSelection(enabled && appearance.picker == side)
                        .disabled(!enabled)
                        .accessibilityLabel("\(side.menuTitle) theme")
                        .accessibilityValue(appearance.themeName(for: side))
                        .accessibilityIdentifier("appearance-picker-\(side.rawValue)")
                        .help(enabled ? "Choose your \(side.menuTitle.lowercased()) theme" : "Used in \(side.menuTitle) or System mode")
                    }
                    .frame(maxWidth: .infinity, alignment: .leading)
                    .opacity(enabled ? 1 : 0.45)
                }
            }
            Text("\(appearance.picker.menuTitle) themes · previewing \(appearance.selectedTheme.name)")
                .font(.caption).foregroundStyle(palette.textSecondary)
            ScrollView {
                if editing, appearance.selectedSource == .yours {
                    ThemeEditor(appearance: appearance)
                } else {
                    VStack(alignment: .leading, spacing: 10) {
                        Text("Built-in").font(.caption).foregroundStyle(palette.textSecondary)
                        grid(entries.builtins)
                        Text("Yours").font(.caption).foregroundStyle(palette.textSecondary)
                        grid(entries.yours, importTile: true)
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
                    ForEach(appearance.draftErrors, id: \.self) { Text($0).foregroundStyle(palette.failure) }
                    DisclosureGroup("Terminal contrast details") {
                        ForEach(terminal.ansiWarnings, id: \.self) { Text($0).foregroundStyle(palette.textSecondary) }
                        ForEach(terminal.dimWarnings, id: \.self) { Text($0).foregroundStyle(palette.textSecondary) }
                    }
                    Text("Explicit colors printed by terminal applications may retain their own appearance.")
                        .foregroundStyle(palette.textSecondary)
                }.font(.caption).frame(maxWidth: .infinity, alignment: .leading)
            }
            if let refusal = appearance.refusal {
                Text(refusal).foregroundStyle(palette.failure).accessibilityIdentifier("appearance-import-refusal")
            }
            if let notice = appearance.notice { Text(notice).foregroundStyle(palette.failure).textSelection(.enabled) }
            HStack {
                if editing, appearance.selectedSource == .yours {
                    Button("Done Editing") { editing = false }
                }
                Text("Apply writes theme: in \(appearance.paths.config.path)")
                    .font(.caption).foregroundStyle(palette.textSecondary).lineLimit(1).truncationMode(.middle)
                Spacer()
                Button("Cancel") { appearance.cancel() }.keyboardShortcut(.cancelAction)
                Button("Apply") { appearance.apply() }.keyboardShortcut(.defaultAction)
                    .disabled(!appearance.canApply)
            }.buttonStyle(WorkOutlineButtonStyle())
        }
        .padding(20).frame(minWidth: 620, minHeight: 560)
        .foregroundStyle(palette.text).background(palette.background)
        .onAppear { appearance.beginPreview() }
        .onChange(of: appearance.chooserOpen) { _, open in if !open { editing = false; dismiss() } }
        .onDisappear { if appearance.chooserOpen { appearance.cancel() } }
    }

    private func grid(_ entries: [ThemeEntry], importTile: Bool = false) -> some View {
        LazyVGrid(columns: [GridItem(.adaptive(minimum: 150))], alignment: .leading, spacing: 12) {
            ForEach(entries) { entry in tile(entry) }
            if importTile {
                Button("Import file…", action: importTheme)
                    .buttonStyle(WorkOutlineButtonStyle())
                    .frame(maxWidth: .infinity, minHeight: 60)
                    .help("Add a Ghostty theme file to your \(appearance.picker.menuTitle.lowercased()) themes")
                    .accessibilityIdentifier("appearance-import-theme")
            }
        }
    }

    private func tile(_ entry: ThemeEntry) -> some View {
        let theme = entry.theme
        let selected = appearance.selectedName == entry.name
        return VStack(alignment: .leading, spacing: 0) {
            Button { editing = false; appearance.previewTheme(entry.name) } label: {
                // The name doubles as the terminal text sample; the tile's edge shows the app surface.
                VStack(alignment: .leading, spacing: 8) {
                    Text(theme.name).foregroundStyle(theme.terminal.foreground.color)
                    HStack(spacing: 3) {
                        ForEach(0..<8) { i in Rectangle().fill(theme.terminal.indexed[i].color).frame(height: 8) }
                    }
                }
                .padding(8)
                .frame(maxWidth: .infinity, alignment: .leading)
                .background(theme.terminal.background.color)
                .padding(10)
                .contentShape(Rectangle()) // The whole tile, gaps included, selects the theme.
            }
            .buttonStyle(.plain)
            .accessibilityIdentifier("appearance-theme-\(entry.name)")
            if selected {
                HStack(spacing: 6) {
                    Button("Customize") { appearance.customize(); editing = true }
                        .help(entry.source == .yours ? "Edit your theme" : "Make your own copy of \(entry.name)")
                        .accessibilityIdentifier("appearance-customize-theme")
                    if appearance.canDelete {
                        Button(appearance.deleteRevertsToBuiltin ? "Revert to built-in" : "Delete") { appearance.deleteTheme() }
                            .accessibilityIdentifier("appearance-delete-theme")
                    }
                }
                .font(.caption).buttonStyle(WorkOutlineButtonStyle())
                .padding([.horizontal, .bottom], 10)
            }
        }
        .frame(maxWidth: .infinity, alignment: .leading)
        .background(theme.palette.background)
        .workSelection(selected)
        .workFocusRing()
    }

    private func importTheme() {
        let panel = NSOpenPanel()
        panel.canChooseDirectories = false
        panel.allowsMultipleSelection = false
        panel.prompt = "Import Theme"
        panel.begin { response in
            guard response == .OK, let url = panel.url else { return }
            appearance.importTheme(url)
        }
    }
}
