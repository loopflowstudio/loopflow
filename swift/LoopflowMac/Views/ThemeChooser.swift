import AppKit
import SwiftUI
import Loopflow

struct ThemeChooser: View {
    let appearance: AppearanceStore
    @State private var editing = false
    @Environment(\.dismiss) private var dismiss
    @Environment(\.palette) private var palette

    var body: some View {
        // System shows both sides; Light or Dark shows only its own.
        let sides: [AppearanceMode] = appearance.current.mode == .system ? [.light, .dark] : [appearance.current.mode]
        let editingYours = editing && appearance.selectedSource == .yours
        let selected = appearance.selectedTheme.name
        VStack(alignment: .leading, spacing: Spacing.lg) {
            Text("Choose Theme").serif(.title).foregroundStyle(palette.emphasisInk)
            Picker("Mode", selection: Binding(get: { appearance.current.mode }, set: { appearance.previewMode($0) })) {
                ForEach(AppearanceMode.allCases, id: \.self) { mode in Text(mode.menuTitle).tag(mode) }
            }
            .pickerStyle(.segmented)
            .labelsHidden()
            .frame(width: 180)
            .accessibilityIdentifier("appearance-mode")
            Text("Previewing \(selected)")
                .font(Typography.meta).foregroundStyle(palette.textSecondary)
            ScrollView {
                if editingYours {
                    ThemeEditor(appearance: appearance)
                } else {
                    VStack(alignment: .leading, spacing: Spacing.sm) {
                        ForEach(sides, id: \.self) { side in
                            let entries = appearance.entries(for: side)
                            Text("\(side.menuTitle) themes · \(appearance.themeName(for: side))")
                                .font(Typography.textStrong)
                                .accessibilityIdentifier("appearance-heading-\(side.rawValue)")
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
            ForEach([appearance.readFailure, appearance.notice].compactMap { $0 }, id: \.self) {
                Text($0).foregroundStyle(palette.failure).textSelection(.enabled)
            }
            HStack {
                if editingYours {
                    Button("Done Editing") { editing = false }
                } else {
                    // Here rather than under the chosen card, so choosing a card never moves the grid;
                    // the label names the theme, since System mode rings one card per side.
                    Button("Customize \(selected)") { appearance.customize(); editing = true }
                        .help(appearance.selectedSource == .yours ? "Edit your theme" : "Make your own copy")
                        .accessibilityIdentifier("appearance-customize-theme")
                }
                if appearance.canDelete {
                    Button(appearance.deleteRevertsToBuiltin ? "Revert \(selected) to built-in" : "Delete \(selected)") {
                        appearance.deleteTheme()
                    }
                        .accessibilityIdentifier("appearance-delete-theme")
                }
                if let config = appearance.configPath {
                    DisclosureGroup("Advanced") {
                        Text(config).font(Typography.mono).textSelection(.enabled)
                    }
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
        CardGrid(minimum: 180, spacing: Spacing.md) {
            ForEach(entries) { entry in card(entry, side: side) }
            if importTile { importPlaceholder }
        }
    }

    /// A dashed tile the same shape as a theme card. The file joins the side its own colors say.
    private var importPlaceholder: some View {
        Button { importTheme() } label: {
            VStack(alignment: .leading, spacing: Spacing.sm) {
                Text("Import file…").foregroundStyle(palette.textSecondary)
                Color.clear.frame(height: 8)
            }
            .padding(Spacing.sm)
            .frame(maxWidth: .infinity, alignment: .leading)
            .padding(Spacing.sm)
            .contentShape(Rectangle())
            .overlay(RoundedRectangle(cornerRadius: CornerRadius.row)
                .strokeBorder(palette.borderStrong, style: StrokeStyle(lineWidth: 1, dash: [4, 3])))
        }
        .buttonStyle(.plain)
        .help("Add a Ghostty theme file to your themes")
        .accessibilityIdentifier("appearance-import-theme")
    }

    /// The theme drawn in its own roles; the chosen card is ringed in the window's focus color.
    private func card(_ entry: ThemeEntry, side: AppearanceMode) -> some View {
        let assigned = appearance.themeName(for: side) == entry.name
        return Button { editing = false; appearance.previewTheme(entry.name) } label: {
            ThemeCard(theme: entry.theme)
                .overlay(RoundedRectangle(cornerRadius: CornerRadius.row)
                    .strokeBorder(assigned ? palette.keyboardFocus : entry.theme.palette.border, lineWidth: assigned ? 2 : 1))
                .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .workFocusRing()
        .accessibilityLabel(entry.name)
        .accessibilityAddTraits(assigned ? .isSelected : [])
        .accessibilityIdentifier("appearance-theme-\(entry.name)")
        .frame(maxWidth: .infinity, alignment: .leading)
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

/// Equal columns at least `minimum` wide, rows as tall as their tallest card, cards top-aligned.
/// Not a `LazyVGrid`: a lazy grid drew no cards after Settings was closed and reopened.
private struct CardGrid: Layout {
    let minimum: CGFloat
    let spacing: CGFloat

    private func columns(_ width: CGFloat) -> (count: Int, width: CGFloat) {
        let count = max(1, Int((width + spacing) / (minimum + spacing)))
        return (count, (width - spacing * CGFloat(count - 1)) / CGFloat(count))
    }

    /// Each row's cards and its height: the tallest card at the column width.
    private func rows(_ subviews: Subviews, _ count: Int, _ column: CGFloat) -> [(cards: Subviews.SubSequence, height: CGFloat)] {
        stride(from: 0, to: subviews.count, by: count).map { start in
            let cards = subviews[start..<min(start + count, subviews.count)]
            return (cards, cards.map { $0.sizeThatFits(ProposedViewSize(width: column, height: nil)).height }.max() ?? 0)
        }
    }

    func sizeThatFits(proposal: ProposedViewSize, subviews: Subviews, cache: inout ()) -> CGSize {
        // An ideal-size question proposes nil or infinity; answer with one column.
        let width = proposal.width.flatMap { $0.isFinite ? $0 : nil } ?? minimum
        let (count, column) = columns(width)
        let heights = rows(subviews, count, column).map(\.height)
        return CGSize(width: width, height: heights.reduce(0, +) + spacing * CGFloat(max(0, heights.count - 1)))
    }

    func placeSubviews(in bounds: CGRect, proposal: ProposedViewSize, subviews: Subviews, cache: inout ()) {
        let (count, column) = columns(bounds.width)
        var y = bounds.minY
        for (cards, height) in rows(subviews, count, column) {
            for (index, subview) in cards.enumerated() {
                let x = bounds.minX + CGFloat(index) * (column + spacing)
                subview.place(at: CGPoint(x: x, y: y), proposal: ProposedViewSize(width: column, height: nil))
            }
            y += height + spacing
        }
    }
}

/// A theme where its colors land in the app: the title bar, a heading, three status
/// words, ANSI 1–6, a selected row and a primary button. It reads the theme it is given,
/// not the environment, so every card in the chooser shows its own theme.
struct ThemeCard: View {
    let theme: ThemeDefinition

    var body: some View {
        let palette = theme.palette
        VStack(alignment: .leading, spacing: 0) {
            palette.titlebarBackground.frame(height: Spacing.md)
            VStack(alignment: .leading, spacing: Spacing.sm) {
                Text(theme.name).font(Typography.textStrong).foregroundStyle(palette.emphasisInk).lineLimit(1)
                HStack(spacing: Spacing.sm) {
                    Text("Busy").foregroundStyle(palette.working)
                    Text("Blocked").foregroundStyle(palette.warning)
                    Text("Error").foregroundStyle(palette.failure)
                }
                .font(Typography.meta)
                HStack(spacing: Spacing.xxs) {
                    ForEach(1..<7) { index in Circle().fill(theme.terminal.indexed[index].color).frame(width: 8, height: 8) }
                }
                HStack(spacing: Spacing.xs) {
                    Text("Selected row").foregroundStyle(palette.workSelectedText)
                        .padding(.horizontal, Spacing.xs).padding(.vertical, Spacing.xxs)
                        .background(palette.workSelectedFill, in: RoundedRectangle(cornerRadius: CornerRadius.row))
                    Text("Run").foregroundStyle(palette.accentText)
                        .padding(.horizontal, Spacing.sm).padding(.vertical, Spacing.xxs)
                        .background(palette.accent, in: RoundedRectangle(cornerRadius: CornerRadius.control))
                }
                .font(Typography.meta).lineLimit(1).fixedSize()
            }
            .padding(Spacing.sm)
        }
        .frame(maxWidth: .infinity, alignment: .leading)
        .background(palette.background)
        .clipShape(RoundedRectangle(cornerRadius: CornerRadius.row))
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
