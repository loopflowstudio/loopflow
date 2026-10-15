import AppKit
import SwiftUI
import Loopflow

struct ThemeChooser: View {
    let appearance: AppearanceStore
    @Environment(\.dismiss) private var dismiss
    @Environment(\.palette) private var palette

    var body: some View {
        let terminal = appearance.selectedTheme.terminal
        VStack(alignment: .leading, spacing: 16) {
            Text("Choose Theme").font(Typography.sectionTitle())
            ScrollView {
                LazyVGrid(columns: [GridItem(.adaptive(minimum: 150))], spacing: 12) {
                    Button { appearance.preview("system") } label: {
                        Label("System", systemImage: "desktopcomputer")
                            .frame(maxWidth: .infinity, minHeight: 84)
                    }
                    .buttonStyle(WorkOutlineButtonStyle())
                    .workSelection(appearance.selectedID == "system")
                    ForEach(appearance.themes) { theme in
                        Button { appearance.preview(theme.id) } label: {
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
                    ForEach(terminal.readabilityErrors, id: \.self) { Text($0).foregroundStyle(palette.failure) }
                    ForEach(terminal.ansiWarnings, id: \.self) { Text($0).foregroundStyle(palette.textSecondary) }
                    ForEach(terminal.dimWarnings, id: \.self) { Text($0).foregroundStyle(palette.textSecondary) }
                    Text("Explicit colors printed by terminal applications may retain their own appearance.")
                        .foregroundStyle(palette.textSecondary)
                }.font(.caption).frame(maxWidth: .infinity, alignment: .leading)
            }
            if let notice = appearance.notice { Text(notice).foregroundStyle(palette.failure).textSelection(.enabled) }
            HStack {
                Button("Import Ghostty Theme…", action: importTheme)
                Spacer()
                Button("Cancel") { appearance.cancel() }.keyboardShortcut(.cancelAction)
                Button("Apply") { appearance.apply() }.keyboardShortcut(.defaultAction)
                    .disabled(!terminal.readabilityErrors.isEmpty)
            }.buttonStyle(WorkOutlineButtonStyle())
        }
        .padding(20).frame(minWidth: 560, minHeight: 450)
        .foregroundStyle(palette.text).background(palette.background)
        .onAppear { appearance.beginPreview() }
        .onChange(of: appearance.chooserOpen) { _, open in if !open { dismiss() } }
        .onDisappear { if appearance.chooserOpen { appearance.cancel() } }
    }

    private func importTheme() {
        let panel = NSOpenPanel()
        panel.canChooseDirectories = false
        panel.allowsMultipleSelection = false
        panel.prompt = "Import"
        panel.begin { response in
            guard response == .OK, let url = panel.url else { return }
            appearance.importTheme(url)
        }
    }
}
