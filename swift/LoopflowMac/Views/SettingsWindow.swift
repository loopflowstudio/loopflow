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
    var search = ""

    var matches: [SettingsPane] {
        let query = search.trimmingCharacters(in: .whitespacesAndNewlines)
        return SettingsPane.allCases.filter { query.isEmpty || $0.rawValue.localizedCaseInsensitiveContains(query) }
    }

    mutating func open(_ pane: SettingsPane) {
        self.pane = pane
        search = ""
    }
}

struct SettingsWindow: View {
    @Binding var navigation: SettingsNavigation
    let appearance: AppearanceStore
    let checkout: String?
    let portfolio: PortfolioService
    @State var mobile = MobileSetup()
    @State private var scope = ScopedSettingsModel()
    @State private var waves: [Wave] = []
    @State private var notices = NoticeCenter()
    @Environment(\.undoManager) private var undoManager

    var body: some View {
        NavigationSplitView {
            List(selection: Binding<SettingsPane?>(
                get: { navigation.pane },
                set: { if let pane = $0 { navigation.open(pane) } }
            )) {
                let panes = navigation.matches
                ForEach(["App", "Workflow", "Infrastructure"], id: \.self) { group in
                    sidebarSection(group, panes: panes.filter { $0.group == group })
                }
            }
            .listStyle(.sidebar)
            .overlay {
                if navigation.matches.isEmpty { EmptyState(title: "No matching panes") }
            }
            .searchable(text: $navigation.search, placement: .sidebar, prompt: "Search")
            .frame(minWidth: 210)
            .navigationSplitViewColumnWidth(min: 210, ideal: 220, max: 280)
        } detail: {
            Group {
                switch navigation.pane {
                case .general, .agents, .context, .git, .machines:
                    ScopedSettings(model: scope, pane: navigation.pane,
                        checkouts: Array(Set(portfolio.repos.map(\.path) + [checkout, scope.checkout].compactMap { $0 })).sorted(),
                        waves: waves, notices: notices)
                case .appearance: ThemeChooser(appearance: appearance, notices: notices)
                case .shortcuts: KeyboardShortcutsView()
                case .mobile: MobileSettings(setup: mobile)
                }
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
