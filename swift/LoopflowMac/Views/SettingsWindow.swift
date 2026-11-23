import Loopflow
import SwiftUI

enum SettingsPane: String, CaseIterable, Identifiable {
    case general = "General"
    case appearance = "Appearance"
    case shortcuts = "Shortcuts"
    case agents = "Agents"
    case machines = "Machines"
    case mobile = "Mobile"
    case project = "Project & Wave Config"

    var id: Self { self }

    var symbol: String {
        switch self {
        case .general: "gearshape"
        case .appearance: "circle.lefthalf.filled"
        case .shortcuts: "keyboard"
        case .agents: "person.2"
        case .machines: "desktopcomputer"
        case .mobile: "iphone"
        case .project: "folder"
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
    @State var personal = ScopedSettingsModel()
    @State var project = ScopedSettingsModel()
    @State private var waves: [Wave] = []
    @State private var notices = NoticeCenter()
    @Environment(\.undoManager) private var undoManager

    var body: some View {
        NavigationSplitView {
            VStack(spacing: 0) {
                let matches = navigation.matches(in: personal.catalogue)
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
                .padding(.horizontal, Spacing.md)
                .padding(.vertical, Spacing.sm)
                .background(.quaternary, in: Capsule())
                .overlay { Capsule().strokeBorder(.separator) }
                .padding(Spacing.sm)
                if navigation.isSearching {
                    searchResults(matches)
                } else {
                    List(selection: Binding<SettingsPane?>(
                        get: { navigation.pane },
                        set: { if let pane = $0 { navigation.open(pane) } }
                    )) {
                        // Separate rows, so selecting a pane never highlights the divider above it.
                        ForEach([SettingsPane.general, .appearance, .shortcuts]) { sidebarRow($0) }
                        Divider().selectionDisabled().accessibilityHidden(true)
                        ForEach([SettingsPane.agents, .machines, .mobile]) { sidebarRow($0) }
                        Divider().selectionDisabled().accessibilityHidden(true)
                        sidebarRow(.project)
                    }
                    .listStyle(.sidebar)
                }
            }
            .frame(minWidth: 210)
            .toolbar(removing: .sidebarToggle)
            .navigationSplitViewColumnWidth(min: 210, ideal: 220, max: 280)
        } detail: {
            Group {
                switch navigation.pane {
                case .general, .agents, .machines, .project:
                    ScopedSettings(model: model(for: navigation.pane), pane: navigation.pane,
                        checkouts: Array(Set(portfolio.repos.map(\.path) + [checkout, project.checkout].compactMap { $0 })).sorted(),
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
        .task(id: personal.observation) { await personal.observe() }
        .onChange(of: checkout ?? portfolio.repos.first?.path, initial: true) { _, path in
            if project.checkout == nil { project.select(checkout: path) }
        }
        .task(id: project.observation) {
            if project.checkout != nil { await project.observe() }
        }
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
        let scope = model(for: navigation.pane)
        if personal.catalogue.contains(where: { $0.key == key && $0.pane != .appearance }),
           scope.reading.value?.settings[key] == nil { return nil }
        return key
    }

    private func model(for pane: SettingsPane) -> ScopedSettingsModel {
        pane == .project ? project : personal
    }

    private func open(_ match: SettingsMatch) {
        if case .setting(let definition, .project) = match {
            if definition.applicability == "repository", project.wave != nil {
                project.select(checkout: project.checkout)
            } else if definition.applicability != "launch" {
                project.target = .general
            }
        }
        navigation.open(match.pane, row: match.key)
    }

    private func move(_ offset: Int, in matches: [SettingsMatch]) -> KeyPress.Result {
        guard !matches.isEmpty else { return .ignored }
        navigation.move(offset, in: matches)
        return .handled
    }

    private func searchResults(_ matches: [SettingsMatch]) -> some View {
        let selection = navigation.selection(in: matches)?.id
        return ScrollViewReader { proxy in
            List {
                let failures = [personal, project].compactMap { $0.reading.failure }
                ForEach(Array(Set(failures)).sorted(), id: \.self) { error in
                    Text(error).foregroundStyle(.secondary)
                    Button("Refresh settings") {
                        for scope in [personal, project] where scope.reading.failure == error { scope.refresh() }
                    }
                }
                ForEach(matches) { result in
                    SettingsSearchRow(result: result, scope: model(for: result.pane), mobile: mobile, notices: notices,
                                      manager: undoManager, selected: result.id == selection) {
                        open(result)
                    }
                    .id(result.id)
                    .listRowBackground(result.id == selection ? Color.accentColor.opacity(0.18) : .clear)
                }
                if personal.catalogue.isEmpty && personal.reading.isLoading {
                    ProgressView("Loading settings").controlSize(.small)
                } else if matches.isEmpty {
                    EmptyState(title: "No matching settings").environment(\.compactPanes, true)
                }
            }
            .listStyle(.sidebar)
            .onChange(of: selection) { _, id in if let id { proxy.scrollTo(id) } }
        }
    }

    private func sidebarRow(_ pane: SettingsPane) -> some View {
        Label {
            HStack {
                Text(pane.rawValue)
                if pane == .mobile && mobile.ready {
                    Spacer()
                    Circle().fill(.green).frame(width: 7, height: 7)
                        .accessibilityLabel("Ready to connect")
                        .accessibilityIdentifier("mobile-ready")
                }
                if pane == .project && project.file?.hasOverrides == true {
                    Spacer()
                    Circle().fill(.secondary).frame(width: 7, height: 7)
                        .accessibilityLabel("Settings saved in this file")
                }
            }
        } icon: {
            Image(systemName: pane.symbol)
                .symbolRenderingMode(.monochrome)
                .foregroundStyle(.primary)
        }
        .tag(pane)
    }
}
