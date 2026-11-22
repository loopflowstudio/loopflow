import AppKit
import SwiftUI
import Loopflow

struct ThemeChooser: View {
    let appearance: AppearanceStore
    @State private var editing = false
    @Environment(\.dismiss) private var dismiss
    @Environment(\.palette) private var palette

    var body: some View {
        // System shows both pickers; Light or Dark shows only its own.
        let sides: [AppearanceMode] = appearance.current.mode == .system ? [.light, .dark] : [appearance.current.mode]
        VStack(alignment: .leading, spacing: Spacing.lg) {
            Text("Choose Theme").serif(.title).foregroundStyle(palette.emphasisInk)
            HStack(alignment: .bottom, spacing: Spacing.lg) {
                Picker("Mode", selection: Binding(get: { appearance.current.mode }, set: { appearance.previewMode($0) })) {
                    ForEach(AppearanceMode.allCases, id: \.self) { mode in Text(mode.menuTitle).tag(mode) }
                }
                .pickerStyle(.segmented)
                .labelsHidden()
                .frame(width: 180)
                .accessibilityIdentifier("appearance-mode")
                ForEach([AppearanceMode.light, .dark], id: \.self) { side in
                    let enabled = appearance.current.mode == .system || appearance.current.mode == side
                    VStack(alignment: .leading, spacing: Spacing.xs) {
                        Text("\(side.menuTitle) theme").font(Typography.meta).foregroundStyle(palette.textSecondary)
                        Button { editing = false; appearance.showPicker(side) } label: {
                            Text(appearance.themeName(for: side))
                                .lineLimit(1).truncationMode(.tail)
                                .frame(maxWidth: .infinity, minHeight: 28, alignment: .leading)
                        }
                        .buttonStyle(LoopflowButtonStyle())
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
            Text("Previewing \(appearance.selectedTheme.name)")
                .font(Typography.meta).foregroundStyle(palette.textSecondary)
            HStack(alignment: .top, spacing: Spacing.lg) {
                ScrollView {
                    if editing, appearance.selectedSource == .yours {
                        ThemeEditor(appearance: appearance)
                    } else {
                        VStack(alignment: .leading, spacing: Spacing.sm) {
                            ForEach(sides, id: \.self) { side in
                                let entries = appearance.entries(for: side)
                                if sides.count > 1 { Text("\(side.menuTitle) themes").font(Typography.textStrong) }
                                Text("Built-in").font(Typography.meta).foregroundStyle(palette.textSecondary)
                                grid(entries.builtins, side: side)
                                Text("Yours").font(Typography.meta).foregroundStyle(palette.textSecondary)
                                grid(entries.yours, side: side, importTile: true)
                            }
                        }
                    }
                    VStack(alignment: .leading, spacing: Spacing.xs) {
                        ForEach(appearance.diagnostics, id: \.self) { Text($0) }
                        ForEach(appearance.draftErrors, id: \.self) { Text($0).foregroundStyle(palette.failure) }
                    }.font(Typography.meta).frame(maxWidth: .infinity, alignment: .leading)
                }
                ThemeSample(theme: appearance.selectedTheme)
            }
            if let refusal = appearance.refusal {
                Text(refusal).foregroundStyle(palette.failure).accessibilityIdentifier("appearance-import-refusal")
            }
            if let notice = appearance.notice { Text(notice).foregroundStyle(palette.failure).textSelection(.enabled) }
            HStack {
                if editing, appearance.selectedSource == .yours {
                    Button("Done Editing") { editing = false }
                }
                DisclosureGroup("Advanced") {
                    Text(appearance.paths.config.path).font(Typography.mono).textSelection(.enabled)
                }
                Spacer()
                Button("Cancel") { appearance.cancel() }.disabled(appearance.saving).keyboardShortcut(.cancelAction)
                Button("Apply") { Task { await appearance.apply() } }.keyboardShortcut(.defaultAction)
                    .buttonStyle(LoopflowButtonStyle(.accent))
                    .disabled(!appearance.canApply)
            }.buttonStyle(LoopflowButtonStyle())
        }
        .disabled(appearance.saving)
        .padding(Spacing.xl).frame(minWidth: 700, minHeight: 560)
        .foregroundStyle(palette.text).background(palette.background)
        .onAppear { appearance.beginPreview() }
        .onChange(of: appearance.chooserOpen) { _, open in if !open { editing = false; dismiss() } }
        .onDisappear { if appearance.chooserOpen { appearance.cancel() } }
    }

    private func grid(_ entries: [ThemeEntry], side: AppearanceMode, importTile: Bool = false) -> some View {
        LazyVGrid(columns: [GridItem(.adaptive(minimum: 150))], alignment: .leading, spacing: Spacing.md) {
            ForEach(entries) { entry in tile(entry, side: side) }
            if importTile { importPlaceholder(side: side) }
        }
    }

    /// A dashed tile the same shape as a theme tile.
    private func importPlaceholder(side: AppearanceMode) -> some View {
        Button { importTheme(side: side) } label: {
            VStack(alignment: .leading, spacing: Spacing.sm) {
                Text("Import file…").foregroundStyle(palette.textSecondary)
                Color.clear.frame(height: 8)
            }
            .padding(Spacing.sm)
            .frame(maxWidth: .infinity, alignment: .leading)
            .padding(Spacing.sm)
            .contentShape(Rectangle())
            .overlay(Rectangle().strokeBorder(palette.borderStrong, style: StrokeStyle(lineWidth: 1, dash: [4, 3])))
        }
        .buttonStyle(.plain)
        .help("Add a Ghostty theme file to your \(side.menuTitle.lowercased()) themes")
        .accessibilityIdentifier(side == .dark ? "appearance-import-theme-dark" : "appearance-import-theme")
    }

    private func tile(_ entry: ThemeEntry, side: AppearanceMode) -> some View {
        let theme = entry.theme
        let assigned = appearance.themeName(for: side) == entry.name
        let selected = assigned && appearance.picker == side
        return VStack(alignment: .leading, spacing: 0) {
            Button { editing = false; appearance.showPicker(side); appearance.previewTheme(entry.name) } label: {
                // The name doubles as the terminal text sample; the tile's edge shows the app surface.
                VStack(alignment: .leading, spacing: Spacing.sm) {
                    Text(theme.name).foregroundStyle(theme.terminal.foreground.color)
                    HStack(spacing: Spacing.xxs) {
                        ForEach(0..<8) { i in Rectangle().fill(theme.terminal.indexed[i].color).frame(height: 8) }
                    }
                }
                .padding(Spacing.sm)
                .frame(maxWidth: .infinity, alignment: .leading)
                .background(theme.terminal.background.color)
                .padding(Spacing.sm)
                .contentShape(Rectangle()) // The whole tile, gaps included, selects the theme.
            }
            .buttonStyle(.plain)
            .accessibilityIdentifier("appearance-theme-\(entry.name)")
            if selected {
                // Labels never wrap mid-word; a narrow tile stacks the buttons instead.
                ViewThatFits(in: .horizontal) {
                    HStack(spacing: Spacing.xs) { themeActions(entry) }
                    VStack(alignment: .leading, spacing: Spacing.xs) { themeActions(entry) }
                }
                .font(Typography.meta).buttonStyle(LoopflowButtonStyle())
                .padding([.horizontal, .bottom], Spacing.sm)
            }
        }
        .frame(maxWidth: .infinity, alignment: .leading)
        .background(theme.palette.background)
        .workSelection(assigned)
        .workFocusRing()
    }

    @ViewBuilder
    private func themeActions(_ entry: ThemeEntry) -> some View {
        Button("Customize") { appearance.customize(); editing = true }
            .help(entry.source == .yours ? "Edit your theme" : "Make your own copy of \(entry.name)")
            .accessibilityIdentifier("appearance-customize-theme")
            .fixedSize()
        if appearance.canDelete {
            Button(appearance.deleteRevertsToBuiltin ? "Revert to built-in" : "Delete") { appearance.deleteTheme() }
                .accessibilityIdentifier("appearance-delete-theme")
                .fixedSize()
        }
    }

    private func importTheme(side: AppearanceMode) {
        appearance.showPicker(side)
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

/// The previewed theme where its colors land in the app: a title bar, a
/// selected Task row, the status words on the sidebar's surface and one
/// terminal line. It reads the theme it is given, not the environment, so it
/// shows the theme under the pointer's last click before Apply.
struct ThemeSample: View {
    let theme: ThemeDefinition

    var body: some View {
        let palette = theme.palette
        VStack(alignment: .leading, spacing: 0) {
            Text("loopflow").font(Typography.textStrong).foregroundStyle(palette.titlebarText)
                .padding(.horizontal, Spacing.md)
                .frame(maxWidth: .infinity, minHeight: HitTarget.comfortable, alignment: .leading)
                .background(palette.titlebarBackground)
            VStack(alignment: .leading, spacing: Spacing.sm) {
                HStack(spacing: Spacing.sm) {
                    Text("Selected Work").font(Typography.textStrong).foregroundStyle(palette.workSelectedText)
                    Spacer(minLength: 0)
                    Text("Blocked since 12:07").font(Typography.meta).monospacedDigit()
                        .foregroundStyle(palette.workSelectedSecondaryText)
                }
                .padding(.horizontal, Spacing.sm)
                .frame(minHeight: HitTarget.comfortable)
                .background(palette.workSelectedFill, in: RoundedRectangle(cornerRadius: CornerRadius.row))
                HStack(spacing: Spacing.sm) {
                    Text("Working").foregroundStyle(palette.working)
                    Text("Available").foregroundStyle(palette.available)
                    Text("Blocked").foregroundStyle(palette.warning)
                    Text("Failed").foregroundStyle(palette.failure)
                    Text("Passed").foregroundStyle(palette.success)
                }
                .font(Typography.text).lineLimit(1).fixedSize()
                .padding(.horizontal, Spacing.sm)
            }
            .padding(Spacing.sm)
            .frame(maxWidth: .infinity, alignment: .leading)
            .background(palette.surfaceMuted)
            Text(verbatim: "$ lf task status LOO-515").font(Typography.mono)
                .foregroundStyle(theme.terminal.foreground.color)
                .padding(Spacing.md)
                .frame(maxWidth: .infinity, alignment: .leading)
                .background(theme.terminal.background.color)
        }
        // Wide enough for the five status words on one line.
        .frame(width: 280)
        .clipShape(RoundedRectangle(cornerRadius: CornerRadius.row))
        .overlay(RoundedRectangle(cornerRadius: CornerRadius.row).strokeBorder(palette.border, lineWidth: 1))
        .accessibilityElement(children: .contain)
        .accessibilityLabel("Sample of \(theme.name)")
        .accessibilityIdentifier("appearance-sample")
    }
}

struct KeyboardShortcutsView: View {
    @Environment(\.palette) private var palette
    /// Window chrome, then every registry action with a default key.
    private var shortcuts: [(String, String)] {
        [("Navigate", "⌘K"), ("Toggle sidebar", "⌘\\"), ("Task actions", "⌘/")]
            + DesktopAction.all.filter { !$0.keys.isEmpty }.map { action in
                let keys = action.keys.map(\.display)
                let display = keys.count > 2 ? "\(keys[0])…\(keys[keys.count - 1])" : keys.joined(separator: " / ")
                return (action.title, display)
            }
            + [("Choose Task file", "⌘P"), ("Save Task file", "⌘S"), ("Open repository", "⇧⌘O"), ("Settings", "⌘,")]
    }
    var body: some View {
        ScrollView {
            Grid(alignment: .leading, horizontalSpacing: Spacing.xxxl, verticalSpacing: Spacing.md) {
                ForEach(shortcuts, id: \.0) { item in
                    GridRow { Text(item.0); Text(item.1).foregroundStyle(palette.textSecondary) }
                }
            }.padding(Spacing.xxl)
        }.frame(minWidth: 500, minHeight: 440)
    }
}
