import SwiftUI
import LoopflowCore

struct AttentionQueueView: View {
    enum QueueFilter: String, CaseIterable, Identifiable {
        case all
        case interactiveOnly = "interactive"
        case escalationsOnly = "escalations"

        var id: String { rawValue }
        var label: String {
            switch self {
            case .all: return "All"
            case .interactiveOnly: return "Interactive"
            case .escalationsOnly: return "Escalations"
            }
        }
    }

    @Environment(RepoState.self) private var repoState
    @Environment(\.palette) private var palette
    @State private var filter: QueueFilter = .all
    @State private var selectedAttentionId: String?

    private var filteredItems: [AttentionItem] {
        repoState.attentionStore.ordered.filter { item in
            switch filter {
            case .all:
                return true
            case .interactiveOnly:
                return item.kind == .interactive
            case .escalationsOnly:
                return item.kind == .algedonic
            }
        }
    }

    private var selectedItem: AttentionItem? {
        selectedAttentionId.flatMap { repoState.attentionStore.item(for: $0) } ?? filteredItems.first
    }

    var body: some View {
        if filteredItems.isEmpty {
            emptyState
        } else {
            HStack(spacing: 0) {
                VStack(alignment: .leading, spacing: Spacing.md) {
                    header
                    ScrollView {
                        LazyVStack(spacing: Spacing.sm) {
                            ForEach(filteredItems) { item in
                                AttentionRow(
                                    item: item,
                                    waveName: repoState.waveStore.wave(for: item.waveId)?.displayName ?? item.waveId,
                                    isSelected: selectedItem?.id == item.id
                                )
                                .onTapGesture {
                                    selectedAttentionId = item.id
                                    Task {
                                        if let updated = try? await repoState.markAttentionViewed(item.id) {
                                            repoState.attentionStore.set(updated)
                                        }
                                    }
                                }
                            }
                        }
                        .padding(.horizontal, Spacing.lg)
                        .padding(.bottom, Spacing.lg)
                    }
                }
                .frame(minWidth: 320, idealWidth: 380)

                Rectangle()
                    .fill(palette.border)
                    .frame(width: 1)

                AttentionDetailView(item: selectedItem)
                    .frame(maxWidth: .infinity, maxHeight: .infinity)
            }
        }
    }

    private var header: some View {
        VStack(alignment: .leading, spacing: Spacing.sm) {
            HStack {
                Text("Queue")
                    .font(Typography.sectionTitle())
                    .foregroundStyle(palette.accent)
                Text("\(filteredItems.count)")
                    .font(Typography.caption())
                    .foregroundStyle(palette.textSecondary)
                    .padding(.horizontal, Spacing.sm)
                    .padding(.vertical, Spacing.xs)
                    .background(palette.surfaceMuted)
                    .clipShape(Capsule())
                Spacer()
            }
            Picker("Filter", selection: $filter) {
                ForEach(QueueFilter.allCases) { filter in
                    Text(filter.label).tag(filter)
                }
            }
            .pickerStyle(.segmented)
        }
        .padding(Spacing.lg)
    }

    private var emptyState: some View {
        ScrollView {
            LazyVStack(alignment: .leading, spacing: Spacing.sm) {
                ForEach(repoState.waves) { wave in
                    waveOverviewRow(wave)
                }
            }
            .padding(Spacing.xl)
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
    }

    private func waveOverviewRow(_ wave: WaveViewModel) -> some View {
        Button {
            repoState.selectedWaveId = wave.id
        } label: {
            HStack(spacing: Spacing.md) {
                Image(systemName: wave.statusIndicator.icon)
                    .foregroundStyle(wave.statusIndicator.color)
                    .frame(width: 16)

                VStack(alignment: .leading, spacing: Spacing.xxs) {
                    Text(wave.displayName)
                        .font(Typography.body())
                        .fontWeight(.medium)
                        .foregroundStyle(palette.text)

                    if let tagline = wave.visionTagline {
                        Text(tagline)
                            .font(Typography.caption())
                            .foregroundStyle(palette.textSecondary)
                            .lineLimit(1)
                    }
                }

                Spacer()

                if let diff = wave.diffIndicator {
                    Text(diff)
                        .font(Typography.caption())
                        .fontWeight(.medium)
                        .foregroundStyle(wave.diffIsPositive ? Color.statusSuccess : Color.statusError)
                }
            }
            .padding(.horizontal, Spacing.lg)
            .padding(.vertical, Spacing.md)
            .background(palette.surface)
            .overlay(
                RoundedRectangle(cornerRadius: CornerRadius.md)
                    .stroke(palette.border, lineWidth: 1)
            )
            .clipShape(RoundedRectangle(cornerRadius: CornerRadius.md))
        }
        .buttonStyle(.plain)
    }
}

private struct AttentionRow: View {
    let item: AttentionItem
    let waveName: String
    let isSelected: Bool

    @Environment(\.palette) private var palette

    var body: some View {
        VStack(alignment: .leading, spacing: Spacing.xs) {
            HStack {
                Label(item.kind.label, systemImage: item.kind.icon)
                    .font(Typography.caption())
                    .foregroundStyle(color)
                Spacer()
                Text(relativeTime)
                    .font(Typography.caption())
                    .foregroundStyle(palette.textSecondary)
            }
            Text(item.title)
                .font(Typography.body())
                .foregroundStyle(palette.text)
            Text(waveName)
                .font(Typography.caption())
                .foregroundStyle(palette.textSecondary)
            if !item.summary.isEmpty {
                Text(item.summary)
                    .font(Typography.caption())
                    .foregroundStyle(palette.textSecondary)
                    .lineLimit(2)
            }
        }
        .padding(Spacing.md)
        .frame(maxWidth: .infinity, alignment: .leading)
        .background(isSelected ? palette.surfaceMuted : palette.surface)
        .overlay(
            RoundedRectangle(cornerRadius: CornerRadius.md)
                .stroke(isSelected ? palette.accent : palette.border, lineWidth: 1)
        )
        .clipShape(RoundedRectangle(cornerRadius: CornerRadius.md))
    }

    private var color: Color {
        switch item.kind {
        case .algedonic: return .statusError
        case .interactive: return .statusSuccess
        }
    }

    private var relativeTime: String {
        let formatter = RelativeDateTimeFormatter()
        formatter.unitsStyle = .short
        return formatter.localizedString(for: item.surfacedAt, relativeTo: Date())
    }
}

struct AttentionDetailView: View {
    let item: AttentionItem?

    @Environment(RepoState.self) private var repoState
    @Environment(\.palette) private var palette

    var body: some View {
        Group {
            if let item {
                ScrollView {
                    VStack(alignment: .leading, spacing: Spacing.lg) {
                        Text(item.title)
                            .font(Typography.sectionTitle())
                            .foregroundStyle(palette.accent)
                        if !item.summary.isEmpty {
                            Text(item.summary)
                                .font(Typography.body())
                                .foregroundStyle(palette.text)
                        }
                        detailBody(item)
                        actionButtons(item)
                    }
                    .padding(Spacing.xl)
                    .frame(maxWidth: .infinity, alignment: .leading)
                }
            } else {
                VStack {
                    Text("Select an item")
                        .font(Typography.sectionTitle())
                        .foregroundStyle(palette.accent)
                }
                .frame(maxWidth: .infinity, maxHeight: .infinity)
            }
        }
        .background(palette.background)
    }

    @ViewBuilder
    private func detailBody(_ item: AttentionItem) -> some View {
        switch item.context {
        case .interactive(let context):
            VStack(alignment: .leading, spacing: Spacing.sm) {
                if let step = context.step {
                    detailLine("Step", step)
                }
                if let designPath = context.designPath {
                    detailLine("Design", designPath)
                }
                if let preview = designPreview(for: item, context: context) {
                    attentionSection("Design preview", accessibilityIdentifier: "attention-design-preview") {
                        Text(renderAttentionMarkdown(preview))
                            .font(Typography.caption())
                            .foregroundStyle(palette.textSecondary)
                            .fixedSize(horizontal: false, vertical: true)
                            .textSelection(.enabled)
                    }
                }
                if let mutationSummary = mutationSummary(for: context) {
                    attentionSection("Proposed mutations", accessibilityIdentifier: "attention-mutation-summary") {
                        Text(renderAttentionMarkdown(mutationSummary))
                            .font(Typography.caption())
                            .foregroundStyle(palette.textSecondary)
                            .fixedSize(horizontal: false, vertical: true)
                            .textSelection(.enabled)
                    }
                }
            }
        case .algedonic(let context):
            VStack(alignment: .leading, spacing: Spacing.sm) {
                if let step = context.step { detailLine("Step", step) }
                if let reason = context.reason { detailLine("Reason", reason) }
                if let error = context.error { detailLine("Error", error) }
                if !context.conflictFiles.isEmpty {
                    Text("Conflicts")
                        .font(Typography.caption())
                        .foregroundStyle(palette.textSecondary)
                    ForEach(context.conflictFiles, id: \.self) { file in
                        Text(file)
                            .font(Typography.code())
                            .foregroundStyle(palette.text)
                    }
                }
            }
        }
    }

    @ViewBuilder
    private func actionButtons(_ item: AttentionItem) -> some View {
        HStack(spacing: Spacing.sm) {
            switch item.context {
            case .interactive(let context):
                if let sessionId = context.terminalSessionId {
                    Button("Open Session") {
                        repoState.openTerminalSession(sessionId)
                    }
                    .buttonStyle(DarkButtonStyle())
                }
            case .algedonic:
                if let wave = repoState.waveStore.wave(for: item.waveId) {
                    Button("Retry") {
                        Task { try? await repoState.restartStep(wave) }
                    }
                    .buttonStyle(DarkButtonStyle())
                }
            }
        }
    }

    private func detailLine(_ label: String, _ value: String) -> some View {
        VStack(alignment: .leading, spacing: Spacing.xxs) {
            Text(label)
                .font(Typography.caption())
                .foregroundStyle(palette.textSecondary)
            Text(value)
                .font(Typography.body())
                .foregroundStyle(palette.text)
        }
    }

    @ViewBuilder
    private func attentionSection<Content: View>(
        _ title: String,
        accessibilityIdentifier: String,
        @ViewBuilder content: () -> Content
    ) -> some View {
        VStack(alignment: .leading, spacing: Spacing.xxs) {
            Text(title)
                .font(Typography.caption())
                .foregroundStyle(palette.textSecondary)
            content()
                .accessibilityIdentifier(accessibilityIdentifier)
        }
    }

    private func designPreview(
        for item: AttentionItem,
        context: InteractiveAttentionContext
    ) -> String? {
        attentionDesignPreviewText(item: item, context: context, repoRoot: repoRoot(for: item))
    }

    private func mutationSummary(for context: InteractiveAttentionContext) -> String? {
        attentionMutationSummary(context)
    }

    private func repoRoot(for item: AttentionItem) -> URL? {
        if let wave = repoState.waveStore.wave(for: item.waveId) {
            return URL(fileURLWithPath: wave.repo)
        }
        return repoState.currentRepo
    }
}

private func renderAttentionMarkdown(_ text: String) -> AttributedString {
    (try? AttributedString(
        markdown: text,
        options: .init(interpretedSyntax: .inlineOnlyPreservingWhitespace)
    )) ?? AttributedString(text)
}

func attentionDesignPreviewText(
    item: AttentionItem,
    context: InteractiveAttentionContext,
    repoRoot: URL?
) -> String? {
    guard context.step == "review-design",
          let designPath = context.designPath,
          let repoRoot,
          let text = try? String(
            contentsOf: repoRoot.appendingPathComponent(designPath),
            encoding: .utf8
          ),
          !text.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
    else {
        return nil
    }
    return previewAttentionText(text, maxLines: 12)
}

func attentionMutationSummary(_ context: InteractiveAttentionContext) -> String? {
    guard context.step == "wave/review",
          let summary = context.mutationSummary?.trimmingCharacters(in: .whitespacesAndNewlines),
          !summary.isEmpty
    else {
        return nil
    }
    return summary
}

private func previewAttentionText(_ text: String, maxLines: Int) -> String {
    let lines = text.components(separatedBy: .newlines)
    if lines.count <= maxLines {
        return text.trimmingCharacters(in: .whitespacesAndNewlines)
    }
    return lines.prefix(maxLines).joined(separator: "\n").trimmingCharacters(in: .whitespacesAndNewlines)
}
