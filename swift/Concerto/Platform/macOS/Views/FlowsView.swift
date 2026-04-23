// Reference view: navigate the flow + step catalog and see "used by" parents.

import LoopflowCore
import SwiftUI

struct FlowsView: View {
    @Environment(RepoState.self) private var repoState
    @Environment(\.palette) private var palette

    @State private var catalog: Catalog?
    @State private var loadError: String?
    @State private var selection: CatalogSelection?

    var body: some View {
        Group {
            if let catalog {
                HSplitView {
                    CatalogPane(
                        catalog: catalog,
                        selection: $selection
                    )
                    .frame(minWidth: 320, idealWidth: 420)
                    .background(palette.background)

                    UsedByPane(
                        catalog: catalog,
                        selection: $selection
                    )
                    .frame(minWidth: 320, idealWidth: 420)
                    .background(palette.surface)
                }
            } else if let loadError {
                VStack(spacing: Spacing.md) {
                    Text("Couldn't load catalog")
                        .font(Typography.sectionTitle())
                    Text(loadError)
                        .font(Typography.caption())
                        .foregroundStyle(.secondary)
                    Button("Retry") { Task { await load() } }
                }
                .frame(maxWidth: .infinity, maxHeight: .infinity)
            } else {
                ProgressView("Loading catalog...")
                    .frame(maxWidth: .infinity, maxHeight: .infinity)
            }
        }
        .task { await load() }
    }

    private func load() async {
        loadError = nil
        do {
            catalog = try await repoState.fetchCatalog()
        } catch {
            loadError = error.localizedDescription
        }
    }
}

// MARK: - Selection

enum CatalogSelection: Hashable {
    case flow(String)
    case step(String)

    var name: String {
        switch self {
        case .flow(let n), .step(let n): return n
        }
    }
}

// MARK: - Catalog (left) pane

private struct CatalogPane: View {
    let catalog: Catalog
    @Binding var selection: CatalogSelection?

    var body: some View {
        let categories = orderedCategories(catalog)

        ScrollView {
            VStack(alignment: .leading, spacing: Spacing.lg) {
                ForEach(categories, id: \.self) { category in
                    CategorySection(
                        category: category,
                        flows: catalog.flows.filter { $0.category == category }
                            .sorted { $0.name < $1.name },
                        steps: catalog.steps.filter { $0.category == category }
                            .sorted { $0.name < $1.name },
                        catalog: catalog,
                        selection: $selection
                    )
                }
            }
            .padding(Spacing.md)
        }
    }

    private func orderedCategories(_ catalog: Catalog) -> [String] {
        // Preserve the canonical order: Build, Govern, Ops, then anything else.
        let canonical = ["Build", "Govern", "Ops"]
        let present = Set(catalog.flows.map { $0.category } + catalog.steps.map { $0.category })
        var result = canonical.filter { present.contains($0) }
        let extras = present.subtracting(canonical).sorted()
        result.append(contentsOf: extras)
        return result
    }
}

private struct CategorySection: View {
    let category: String
    let flows: [CatalogFlow]
    let steps: [CatalogStep]
    let catalog: Catalog
    @Binding var selection: CatalogSelection?

    var body: some View {
        VStack(alignment: .leading, spacing: Spacing.sm) {
            Text(category)
                .font(Typography.sectionTitle(18))
                .padding(.bottom, Spacing.xs)

            if !flows.isEmpty {
                Text("Flows")
                    .font(Typography.caption())
                    .foregroundStyle(.secondary)
                ForEach(flows) { flow in
                    FlowDisclosure(flow: flow, catalog: catalog, selection: $selection)
                }
            }

            if !steps.isEmpty {
                Text("Steps")
                    .font(Typography.caption())
                    .foregroundStyle(.secondary)
                    .padding(.top, Spacing.xs)
                ForEach(steps) { step in
                    SelectableRow(
                        title: step.name,
                        subtitle: step.description,
                        isSelected: selection == .step(step.name),
                        accent: step.source == .repo
                    ) {
                        selection = .step(step.name)
                    }
                }
            }
        }
    }
}

private struct FlowDisclosure: View {
    let flow: CatalogFlow
    let catalog: Catalog
    @Binding var selection: CatalogSelection?

    @State private var expanded = false

    var body: some View {
        VStack(alignment: .leading, spacing: 2) {
            HStack(spacing: Spacing.xs) {
                Button {
                    expanded.toggle()
                } label: {
                    Image(systemName: expanded ? "chevron.down" : "chevron.right")
                        .font(.system(size: 10))
                        .frame(width: 12)
                }
                .buttonStyle(.plain)

                SelectableRow(
                    title: flow.name,
                    subtitle: nil,
                    isSelected: selection == .flow(flow.name),
                    accent: flow.source == .repo,
                    leading: { Text("●").font(.system(size: 8)).foregroundStyle(.secondary) }
                ) {
                    selection = .flow(flow.name)
                }
            }

            if expanded {
                FlowItemList(items: flow.items, catalog: catalog, depth: 1, selection: $selection)
            }
        }
    }
}

private struct FlowItemList: View {
    let items: [CatalogFlowItem]
    let catalog: Catalog
    let depth: Int
    @Binding var selection: CatalogSelection?

    var body: some View {
        VStack(alignment: .leading, spacing: 2) {
            ForEach(Array(items.enumerated()), id: \.offset) { _, item in
                FlowItemRow(item: item, catalog: catalog, depth: depth, selection: $selection)
            }
        }
        .padding(.leading, CGFloat(depth) * 12)
    }
}

private struct FlowItemRow: View {
    let item: CatalogFlowItem
    let catalog: Catalog
    let depth: Int
    @Binding var selection: CatalogSelection?

    @State private var expanded = false

    var body: some View {
        switch item {
        case let .step(name, _):
            SelectableRow(
                title: name,
                subtitle: nil,
                isSelected: selection == .step(name),
                accent: false,
                leading: { Image(systemName: "square.dashed").font(.system(size: 9)) }
            ) {
                selection = .step(name)
            }
        case let .op(command, args):
            HStack(spacing: Spacing.xs) {
                Image(systemName: "terminal").font(.system(size: 9)).foregroundStyle(.secondary)
                Text("op: \(([command] + args).joined(separator: " "))")
                    .font(Typography.code(11))
                    .foregroundStyle(.secondary)
            }
        case let .flowRef(name):
            VStack(alignment: .leading, spacing: 2) {
                HStack(spacing: Spacing.xs) {
                    Button { expanded.toggle() } label: {
                        Image(systemName: expanded ? "chevron.down" : "chevron.right")
                            .font(.system(size: 10))
                            .frame(width: 12)
                    }
                    .buttonStyle(.plain)
                    SelectableRow(
                        title: name,
                        subtitle: nil,
                        isSelected: selection == .flow(name),
                        accent: false,
                        leading: { Text("●").font(.system(size: 8)).foregroundStyle(.secondary) }
                    ) {
                        selection = .flow(name)
                    }
                }
                if expanded, let nested = catalog.flowsByName[name] {
                    FlowItemList(items: nested.items, catalog: catalog, depth: depth + 1, selection: $selection)
                }
            }
        case let .xor(def):
            BranchRow(label: "xor", router: def.router, paths: def.paths, depth: depth, selection: $selection)
        case let .or(def):
            BranchRow(label: "or", router: def.router, paths: def.paths, depth: depth, selection: $selection)
        case let .loop(def):
            VStack(alignment: .leading, spacing: 2) {
                Text("loop")
                    .font(Typography.code(11))
                    .foregroundStyle(.secondary)
                FlowItemList(items: def.steps, catalog: catalog, depth: depth + 1, selection: $selection)
                Text("exit")
                    .font(Typography.caption(10))
                    .foregroundStyle(.secondary)
                    .padding(.leading, CGFloat(depth + 1) * 12)
                BranchRow(label: "", router: def.exit.router, paths: def.exit.paths, depth: depth + 1, selection: $selection)
            }
        case let .and(def):
            VStack(alignment: .leading, spacing: 2) {
                Text("and")
                    .font(Typography.code(11))
                    .foregroundStyle(.secondary)
                FlowItemList(items: def.branches, catalog: catalog, depth: depth + 1, selection: $selection)
            }
        }
    }
}

private struct BranchRow: View {
    let label: String
    let router: String?
    let paths: [String: CatalogXorPath]
    let depth: Int
    @Binding var selection: CatalogSelection?

    var body: some View {
        VStack(alignment: .leading, spacing: 2) {
            HStack(spacing: Spacing.xs) {
                if !label.isEmpty {
                    Text(label).font(Typography.code(11)).foregroundStyle(.secondary)
                }
                if let router {
                    Text("router: \(router)").font(Typography.code(11)).foregroundStyle(.secondary)
                }
            }
            ForEach(paths.keys.sorted(), id: \.self) { key in
                if let path = paths[key] {
                    HStack(spacing: Spacing.xs) {
                        Image(systemName: "arrow.triangle.branch").font(.system(size: 9)).foregroundStyle(.secondary)
                        Text(key).font(Typography.code(11))
                        if let target = path.flow ?? path.step {
                            Text("→").foregroundStyle(.secondary)
                            Button(target) {
                                selection = path.flow != nil ? .flow(target) : .step(target)
                            }
                            .buttonStyle(.plain)
                        }
                        if !path.description.isEmpty {
                            Text("— \(path.description)").font(Typography.caption(10)).foregroundStyle(.secondary)
                        }
                    }
                    .padding(.leading, 12)
                }
            }
        }
        .padding(.leading, CGFloat(depth) * 12)
    }
}

private struct SelectableRow<Leading: View>: View {
    let title: String
    let subtitle: String?
    let isSelected: Bool
    let accent: Bool
    let leading: () -> Leading
    let onTap: () -> Void

    init(
        title: String,
        subtitle: String?,
        isSelected: Bool,
        accent: Bool,
        @ViewBuilder leading: @escaping () -> Leading = { EmptyView() },
        onTap: @escaping () -> Void
    ) {
        self.title = title
        self.subtitle = subtitle
        self.isSelected = isSelected
        self.accent = accent
        self.leading = leading
        self.onTap = onTap
    }

    var body: some View {
        Button(action: onTap) {
            HStack(spacing: Spacing.xs) {
                leading()
                Text(title)
                    .font(Typography.body(13))
                if accent {
                    Text("repo")
                        .font(Typography.caption(9))
                        .padding(.horizontal, 4)
                        .padding(.vertical, 1)
                        .background(Color.loopflowBurgundy.opacity(0.15))
                        .foregroundStyle(Color.loopflowBurgundy)
                        .clipShape(Capsule())
                }
                if let subtitle, !subtitle.isEmpty {
                    Text(subtitle)
                        .font(Typography.caption(10))
                        .foregroundStyle(.secondary)
                        .lineLimit(1)
                }
                Spacer(minLength: 0)
            }
            .padding(.horizontal, Spacing.xs)
            .padding(.vertical, 2)
            .background(
                RoundedRectangle(cornerRadius: CornerRadius.sm)
                    .fill(isSelected ? Color.accentColor.opacity(0.15) : Color.clear)
            )
            .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
    }
}

// MARK: - Used By (right) pane

private struct UsedByPane: View {
    let catalog: Catalog
    @Binding var selection: CatalogSelection?

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: Spacing.md) {
                if let selection {
                    SelectionDetail(catalog: catalog, selection: selection, currentSelection: $selection)
                } else {
                    Text("Select a flow or step to see what uses it.")
                        .font(Typography.caption())
                        .foregroundStyle(.secondary)
                        .padding(.top, Spacing.xxxl)
                        .frame(maxWidth: .infinity)
                }
            }
            .padding(Spacing.md)
        }
    }
}

private struct SelectionDetail: View {
    let catalog: Catalog
    let selection: CatalogSelection
    @Binding var currentSelection: CatalogSelection?

    var body: some View {
        VStack(alignment: .leading, spacing: Spacing.sm) {
            HStack(spacing: Spacing.xs) {
                Image(systemName: selection.kindIcon)
                    .foregroundStyle(.secondary)
                Text(selection.name)
                    .font(Typography.sectionTitle(20))
            }

            if let description = description(for: selection) {
                Text(description)
                    .font(Typography.body())
                    .foregroundStyle(.secondary)
            }

            Divider().padding(.vertical, Spacing.xs)

            Text("Used by")
                .font(Typography.caption())
                .foregroundStyle(.secondary)

            let parents = catalog.directParents(of: selection.name)
            if parents.isEmpty {
                Text("Nothing references this directly.")
                    .font(Typography.caption())
                    .foregroundStyle(.tertiary)
            } else {
                ForEach(parents) { parent in
                    UsedByNode(
                        flow: parent,
                        catalog: catalog,
                        ancestry: [],
                        currentSelection: $currentSelection
                    )
                }
            }
        }
    }

    private func description(for selection: CatalogSelection) -> String? {
        switch selection {
        case .step(let name):
            return catalog.stepsByName[name]?.description
        case .flow:
            return nil
        }
    }
}

private struct UsedByNode: View {
    let flow: CatalogFlow
    let catalog: Catalog
    let ancestry: [String]
    @Binding var currentSelection: CatalogSelection?

    @State private var expanded = false

    var body: some View {
        let parents = catalog.directParents(of: flow.name)
            .filter { !ancestry.contains($0.name) && $0.name != flow.name }

        VStack(alignment: .leading, spacing: 2) {
            HStack(spacing: Spacing.xs) {
                if !parents.isEmpty {
                    Button { expanded.toggle() } label: {
                        Image(systemName: expanded ? "chevron.down" : "chevron.right")
                            .font(.system(size: 10))
                            .frame(width: 12)
                    }
                    .buttonStyle(.plain)
                } else {
                    Spacer().frame(width: 12)
                }

                Button {
                    currentSelection = .flow(flow.name)
                } label: {
                    HStack(spacing: Spacing.xs) {
                        Image(systemName: "arrow.up").font(.system(size: 9)).foregroundStyle(.secondary)
                        Text(flow.name).font(Typography.body(13))
                        Text(flow.category)
                            .font(Typography.caption(10))
                            .foregroundStyle(.secondary)
                    }
                }
                .buttonStyle(.plain)
            }

            if expanded {
                VStack(alignment: .leading, spacing: 2) {
                    ForEach(parents) { parent in
                        UsedByNode(
                            flow: parent,
                            catalog: catalog,
                            ancestry: ancestry + [flow.name],
                            currentSelection: $currentSelection
                        )
                    }
                }
                .padding(.leading, 18)
            }
        }
    }
}

private extension CatalogSelection {
    var kindIcon: String {
        switch self {
        case .flow: return "circle.fill"
        case .step: return "square.dashed"
        }
    }
}
