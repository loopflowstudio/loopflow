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
        VStack(spacing: Spacing.sm) {
            Text("Nothing needs you.")
                .font(Typography.sectionTitle())
                .foregroundStyle(palette.accent)
            Text("Waves are running.")
                .font(Typography.body())
                .foregroundStyle(palette.textSecondary)
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
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

private struct AttentionDetailView: View {
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
<<<<<<< HEAD
<<<<<<< HEAD
=======
        case .stepFailure(let context):
            VStack(alignment: .leading, spacing: Spacing.sm) {
                if let step = context.step { detailLine("Step", step) }
                if let error = context.error { detailLine("Error", error) }
            }
        case .designReview(let context):
            VStack(alignment: .leading, spacing: Spacing.sm) {
                detailLine("Step", context.step)
                if let designPath = context.designPath {
                    detailLine("Design", designPath)
                }
            }
        case .calibration(let context):
            VStack(alignment: .leading, spacing: Spacing.sm) {
                detailLine("Step", context.step)
                if let chordPath = context.chordPath {
                    detailLine("Chord", chordPath)
                }
            }
=======
>>>>>>> bb36fbcb (attention: collapse kinds to Interactive/Algedonic, add HTTP create/resolve API)
        case .raw(let raw):
            Text(raw)
                .font(Typography.code())
                .foregroundStyle(palette.text)
>>>>>>> 07c9c6ed (attention queue completion: wire interactive steps into attention queue)
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
<<<<<<< HEAD
<<<<<<< HEAD
=======
            case .designReview(let context):
                if let sessionId = context.terminalSessionId {
                    Button("Open Session") {
                        repoState.openTerminalSession(sessionId)
                    }
                    .buttonStyle(DarkButtonStyle())
                }
            case .calibration(let context):
                if let sessionId = context.terminalSessionId {
                    Button("Open Session") {
                        repoState.openTerminalSession(sessionId)
                    }
                    .buttonStyle(DarkButtonStyle())
                }
            case .queueFailure, .raw:
=======
            case .raw:
>>>>>>> bb36fbcb (attention: collapse kinds to Interactive/Algedonic, add HTTP create/resolve API)
                EmptyView()
>>>>>>> 07c9c6ed (attention queue completion: wire interactive steps into attention queue)
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
}
