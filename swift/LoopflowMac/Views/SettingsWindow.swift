import Loopflow
import SwiftUI

enum SettingsPane: String, CaseIterable, Identifiable {
    case general = "General"
    case appearance = "Appearance"
    case shortcuts = "Shortcuts"
    case mobile = "Mobile"
    case configuration = "Configuration"

    var id: Self { self }

    var badge: (symbol: String, color: Color) {
        switch self {
        case .general: ("gearshape.fill", .gray)
        case .appearance: ("paintpalette.fill", .purple)
        case .shortcuts: ("keyboard", .indigo)
        case .mobile: ("iphone", .green)
        case .configuration: ("slider.horizontal.3", .blue)
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
    @State private var notices = NoticeCenter()
    @Environment(\.undoManager) private var undoManager

    var body: some View {
        NavigationSplitView {
            List(selection: Binding<SettingsPane?>(
                get: { navigation.pane },
                set: { if let pane = $0 { navigation.open(pane) } }
            )) {
                let panes = navigation.matches
                sidebarSection("App", panes: panes.filter { $0 != .configuration })
                sidebarSection("Workflow", panes: panes.filter { $0 == .configuration })
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
                case .general: GeneralSettings(notices: notices, undoManager: undoManager)
                case .appearance: ThemeChooser(appearance: appearance)
                case .shortcuts: KeyboardShortcutsView()
                case .mobile: MobileSettings()
                case .configuration: ConfigurationSettingsView()
                }
            }
            .navigationTitle(navigation.pane.rawValue)
            .overlay(alignment: .bottom) { NoticeView(center: notices) }
        }
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
                        Text(pane.rawValue)
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
