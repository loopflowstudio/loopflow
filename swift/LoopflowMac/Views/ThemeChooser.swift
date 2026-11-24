import AppKit
import SwiftUI
import Loopflow

struct ThemeChooser: View {
    let appearance: AppearanceStore
    let notices: NoticeCenter
    var highlightedKey: String?
    @Environment(\.undoManager) private var undoManager
    @Environment(\.palette) private var palette

    var body: some View {
        Form {
            if appearance.readFailure != nil || appearance.notice != nil {
                Section {
                    if let failure = appearance.readFailure { Text(failure).foregroundStyle(palette.failure).textSelection(.enabled) }
                    if let notice = appearance.notice { Text(notice).textSelection(.enabled) }
                }
            }
            if appearance.editing {
                ThemeEditor(appearance: appearance)
                    .settingsRow(SettingsMatch.theme.id, highlighted: highlightedKey)
            } else {
                Section {
                    Picker("Appearance", selection: Binding(get: { appearance.selection.mode }, set: { mode in
                        Task { await appearance.applyMode(mode, notices: notices, manager: undoManager) }
                    })) {
                        ForEach(AppearanceMode.allCases, id: \.self) { Text($0.menuTitle).tag($0) }
                    }
                    .pickerStyle(.segmented)
                    .accessibilityIdentifier("appearance-mode")
                    .settingsRow(SettingsMatch.appearanceMode.id, highlighted: highlightedKey)

                    Button("Customize \(appearance.selectedName)") { appearance.customize() }
                        .accessibilityIdentifier("appearance-customize-theme")
                        .settingsRow(SettingsMatch.theme.id, highlighted: highlightedKey)
                }

                let sides: [AppearanceMode] = appearance.selection.mode == .system ? [.light, .dark] : [appearance.selection.mode]
                ForEach(sides, id: \.self) { side in
                    let entries = appearance.entries(for: side)
                    Section {
                        Text("Built-in").font(Typography.meta)
                        grid(entries.builtins, side: side)
                        Text("Yours").font(Typography.meta)
                        grid(entries.yours, side: side, importTile: true)
                    } header: {
                        Text("\(side.menuTitle) themes · \(appearance.themeName(for: side))")
                            .accessibilityIdentifier("appearance-heading-\(side.rawValue)")
                    }
                }
                if let config = appearance.configPath {
                    Section {
                        LabeledContent("Configuration file") {
                            Text(config).font(Typography.mono).textSelection(.enabled)
                        }
                    }
                }
            }
        }
        .settingsForm(highlighted: highlightedKey)
        .disabled(appearance.saving)
        .onDisappear { if appearance.editing { appearance.cancel() } }
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
        return Button { Task { await appearance.selectTheme(entry.name, notices: notices, manager: undoManager) } } label: {
            ThemeCard(theme: entry.theme)
                .overlay(RoundedRectangle(cornerRadius: CornerRadius.row)
                    .strokeBorder(assigned ? Color.accentColor : entry.theme.palette.border, lineWidth: assigned ? 2 : 1))
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
