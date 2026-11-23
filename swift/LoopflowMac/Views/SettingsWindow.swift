import Loopflow
import SwiftUI

enum SettingsPane: String, CaseIterable, Identifiable {
    case general = "General"
    case appearance = "Appearance"
    case shortcuts = "Shortcuts"
    case mobile = "Mobile"
    case agents = "Agents"
    case context = "Context"
    case git = "Git & Release"
    case machines = "Machines"

    var id: Self { self }

    var group: String {
        switch self {
        case .agents, .context, .git: "Workflow"
        case .machines: "Infrastructure"
        default: "App"
        }
    }

    var badge: (symbol: String, color: Color) {
        switch self {
        case .general: ("gearshape.fill", .gray)
        case .appearance: ("paintpalette.fill", .purple)
        case .shortcuts: ("keyboard", .indigo)
        case .mobile: ("iphone", .green)
        case .context: ("doc.text.fill", .orange)
        case .agents: ("person.2.fill", .blue)
        case .git: ("arrow.triangle.branch", .teal)
        case .machines: ("desktopcomputer", .indigo)
        }
    }
}

struct SettingsNavigation {
    var pane = SettingsPane.general
    var search = "" {
        didSet {
            guard search != oldValue else { return }
            selectedResult = nil
            highlightedKey = nil
        }
    }
    var selectedResult: String?
    var highlightedKey: String?

    var isSearching: Bool { !search.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty }

    func selection(in matches: [SettingsMatch]) -> SettingsMatch? {
        matches.first { $0.id == selectedResult } ?? matches.first
    }

    mutating func move(_ offset: Int, in matches: [SettingsMatch]) {
        guard !matches.isEmpty else { return }
        let current = matches.firstIndex { $0.id == selectedResult } ?? 0
        selectedResult = matches[min(max(current + offset, 0), matches.count - 1)].id
    }

    mutating func open(_ pane: SettingsPane, row: String? = nil) {
        self.pane = pane
        search = ""
        highlightedKey = row
    }
}

struct SettingsWindow: View {
    @Binding var navigation: SettingsNavigation
    let appearance: AppearanceStore
    let checkout: String?
    let portfolio: PortfolioService
    @State var mobile = MobileSetup()
    @State var scope = ScopedSettingsModel()
    @State private var waves: [Wave] = []
    @State private var notices = NoticeCenter()
    @Environment(\.undoManager) private var undoManager

    var body: some View {
        NavigationSplitView {
            VStack(spacing: 0) {
                let matches = navigation.matches(in: scope.catalogue)
                HStack {
                    Image(systemName: "magnifyingglass").foregroundStyle(.secondary)
                    TextField("Search settings", text: $navigation.search)
                        .textFieldStyle(.plain)
                        .onKeyPress(.downArrow) { move(1, in: matches) }
                        .onKeyPress(.upArrow) { move(-1, in: matches) }
                        .onExitCommand { navigation.search = "" }
                        .onSubmit {
                            if let match = navigation.selection(in: matches) { open(match) }
                        }
                    if !navigation.search.isEmpty {
                        Button { navigation.search = "" } label: { Image(systemName: "xmark.circle.fill") }
                            .buttonStyle(.plain).accessibilityLabel("Clear search")
                    }
                }
                .padding(Spacing.sm)
                .background(.quaternary, in: RoundedRectangle(cornerRadius: CornerRadius.control))
                .padding(Spacing.sm)
                if navigation.isSearching {
                    searchResults(matches)
                } else {
                    List(selection: Binding<SettingsPane?>(
                        get: { navigation.pane },
                        set: { if let pane = $0 { navigation.open(pane) } }
                    )) {
                        ForEach(["App", "Workflow", "Infrastructure"], id: \.self) { group in
                            sidebarSection(group, panes: SettingsPane.allCases.filter { $0.group == group })
                        }
                    }
                    .listStyle(.sidebar)
                }
            }
            .frame(minWidth: 210)
            .navigationSplitViewColumnWidth(min: 210, ideal: 220, max: 280)
        } detail: {
            Group {
                switch navigation.pane {
                case .general, .agents, .context, .git, .machines:
                    ScopedSettings(model: scope, pane: navigation.pane,
                        checkouts: Array(Set(portfolio.repos.map(\.path) + [checkout, scope.checkout].compactMap { $0 })).sorted(),
                        waves: waves, notices: notices, highlightedKey: scrollTarget)
                case .appearance: ThemeChooser(appearance: appearance, notices: notices, highlightedKey: scrollTarget)
                case .shortcuts: KeyboardShortcutsView()
                case .mobile: MobileSettings(setup: mobile, highlightedKey: scrollTarget)
                }
            }
            .task(id: scrollTarget) {
                guard let key = scrollTarget else { return }
                do { try await Task.sleep(for: .seconds(1.5)) } catch { return }
                if navigation.highlightedKey == key { navigation.highlightedKey = nil }
            }
            .navigationTitle(navigation.pane.rawValue)
            .overlay(alignment: .bottom) { NoticeView(center: notices) }
        }
        .task(id: scope.observation) { await scope.observe() }
        .task { await mobile.observe() }
        .task { await portfolio.refresh() }
        .task { waves = (try? await RegistryQueryLocal.shared.allWaves()) ?? [] }
        .frame(minWidth: 760, minHeight: 560)
        .environment(\.palette, appearance.effectiveTheme.palette)
        .environment(\.theme, appearance.effectiveTheme)
        .preferredColorScheme(appearance.effectiveTheme.colorScheme)
    }

    private var scrollTarget: String? {
        guard let key = navigation.highlightedKey else { return nil }
        if key == SettingsMatch.appearanceMode.id && appearance.editing { return nil }
        if scope.catalogue.contains(where: { $0.key == key && $0.pane != .appearance }),
           scope.reading.value?.settings[key] == nil { return nil }
        return key
    }

    private func open(_ match: SettingsMatch) { navigation.open(match.pane, row: match.key) }

    private func move(_ offset: Int, in matches: [SettingsMatch]) -> KeyPress.Result {
        guard !matches.isEmpty else { return .ignored }
        navigation.move(offset, in: matches)
        return .handled
    }

    private func searchResults(_ matches: [SettingsMatch]) -> some View {
        let selection = navigation.selection(in: matches)?.id
        return ScrollViewReader { proxy in
            List {
                if let error = scope.reading.failure {
                    Text(error).foregroundStyle(.secondary)
                    Button("Refresh settings") { scope.refresh() }
                }
                ForEach(matches) { result in
                    SettingsSearchRow(result: result, scope: scope, mobile: mobile, notices: notices,
                                      manager: undoManager, selected: result.id == selection) {
                        open(result)
                    }
                    .id(result.id)
                    .listRowBackground(result.id == selection ? Color.accentColor.opacity(0.18) : .clear)
                }
                if scope.catalogue.isEmpty && scope.reading.isLoading {
                    ProgressView("Loading settings").controlSize(.small)
                } else if matches.isEmpty {
                    EmptyState(title: "No matching settings").environment(\.compactPanes, true)
                }
            }
            .listStyle(.sidebar)
            .onChange(of: selection) { _, id in if let id { proxy.scrollTo(id) } }
        }
    }

    @ViewBuilder
    private func sidebarSection(_ title: String, panes: [SettingsPane]) -> some View {
        if !panes.isEmpty {
            Section(title) {
                ForEach(panes) { pane in
                    Label {
                        HStack {
                            Text(pane.rawValue)
                            if pane == .mobile && mobile.ready {
                                Spacer()
                                Circle().fill(.green).frame(width: 7, height: 7)
                                    .accessibilityLabel("Ready to connect")
                                    .accessibilityIdentifier("mobile-ready")
                            }
                            if let file = scope.file, file.hasOverrides(in: pane, catalog: scope.catalogue) {
                                Spacer()
                                Circle().fill(.secondary).frame(width: 7, height: 7)
                                    .accessibilityLabel("Settings saved in this file")
                            }
                        }
                    } icon: {
                        Image(systemName: pane.badge.symbol)
                            .imageScale(.small)
                            .foregroundStyle(.white)
                            .frame(width: 22, height: 22)
                            .background(pane.badge.color, in: RoundedRectangle(cornerRadius: CornerRadius.control))
                    }
                    .tag(pane)
                }
            }
        }
    }
}
