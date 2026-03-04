import SwiftUI
import LoopflowCore

struct SessionContextView: View {
    @Environment(\.palette) private var palette
    @Environment(\.accessibilityReduceMotion) private var reduceMotion

    let snapshot: ContextSnapshot

    @State private var isExpanded = false
    @State private var expandedSources: Set<String> = []

    private var usagePercent: UInt64 {
        guard snapshot.budget > 0 else { return 0 }
        return (snapshot.total * 100) / snapshot.budget
    }

    private var rows: [ContextSourceRow] {
        contextSourceRows(snapshot: snapshot)
    }

    private var segments: [ContextSegment] {
        guard snapshot.total > 0 else { return [] }
        return rows.map { row in
            ContextSegment(
                source: row.source,
                fraction: Double(row.tokens) / Double(snapshot.total),
                color: color(for: row.source)
            )
        }
    }

    var body: some View {
        VStack(alignment: .leading, spacing: Spacing.sm) {
            Button {
                withAnimation(DesignAnimation.standard(reduceMotion)) {
                    isExpanded.toggle()
                    if !isExpanded {
                        expandedSources.removeAll()
                    }
                }
            } label: {
                VStack(alignment: .leading, spacing: Spacing.xs) {
                    GeometryReader { geometry in
                        HStack(spacing: 0) {
                            ForEach(segments) { segment in
                                Rectangle()
                                    .fill(segment.color)
                                    .frame(width: geometry.size.width * segment.fraction)
                            }
                        }
                        .frame(maxWidth: .infinity, alignment: .leading)
                        .background(palette.surfaceMuted)
                        .clipShape(RoundedRectangle(cornerRadius: CornerRadius.sm))
                    }
                    .frame(height: 10)

                    HStack(spacing: Spacing.sm) {
                        Text("\(usagePercent)% of \(formatContextBudget(snapshot.budget))")
                            .font(Typography.caption())
                            .foregroundStyle(palette.textSecondary)
                            .monospacedDigit()
                        Spacer(minLength: 0)
                        Text(formatTokenCount(snapshot.total))
                            .font(Typography.caption())
                            .foregroundStyle(palette.textSecondary)
                            .monospacedDigit()
                        Image(systemName: isExpanded ? "chevron.down" : "chevron.right")
                            .font(Typography.caption())
                            .foregroundStyle(palette.textSecondary)
                    }

                    if !rows.isEmpty {
                        Text(rows.prefix(5).map(\.label).joined(separator: "  "))
                            .font(Typography.caption(11))
                            .foregroundStyle(palette.textSecondary)
                            .lineLimit(1)
                    }
                }
                .frame(maxWidth: .infinity, alignment: .leading)
            }
            .buttonStyle(.plain)
            .accessibilityLabel("Context usage")
            .accessibilityValue("\(usagePercent)% of budget")

            if isExpanded {
                contextDetail
                    .transition(.opacity)
            }
        }
        .padding(Spacing.md)
        .background(palette.surface)
        .clipShape(RoundedRectangle(cornerRadius: CornerRadius.md))
        .overlay(
            RoundedRectangle(cornerRadius: CornerRadius.md)
                .stroke(palette.border, lineWidth: 1)
        )
        .onChange(of: snapshot) { _, _ in
            expandedSources.removeAll()
        }
    }

    private var contextDetail: some View {
        VStack(alignment: .leading, spacing: Spacing.sm) {
            Divider()
            HStack(spacing: Spacing.sm) {
                Text("Context")
                    .font(Typography.sectionTitle(18))
                    .foregroundStyle(palette.accent)
                Spacer(minLength: 0)
                Text("\(usagePercent)% of \(formatContextBudget(snapshot.budget))")
                    .font(Typography.caption())
                    .foregroundStyle(palette.textSecondary)
                    .monospacedDigit()
            }

            LazyVStack(alignment: .leading, spacing: Spacing.sm) {
                ForEach(rows, id: \.source) { row in
                    sourceRow(row)
                }
            }

            Divider()
            HStack(spacing: Spacing.sm) {
                Text("total")
                    .font(Typography.caption())
                    .foregroundStyle(palette.textSecondary)
                Spacer(minLength: 0)
                Text(formatTokenCount(snapshot.total))
                    .font(Typography.caption())
                    .foregroundStyle(palette.textSecondary)
                    .monospacedDigit()
            }
        }
        .padding(.top, Spacing.xs)
    }

    private func sourceRow(_ row: ContextSourceRow) -> some View {
        let documents = contextDocumentSlice(snapshot: snapshot, source: row.source)
        let canExpand = !documents.visible.isEmpty || documents.remainingCount > 0
        let isSourceExpanded = expandedSources.contains(row.source)

        return VStack(alignment: .leading, spacing: Spacing.xs) {
            Button {
                guard canExpand else { return }
                withAnimation(DesignAnimation.standard(reduceMotion)) {
                    if !expandedSources.insert(row.source).inserted {
                        expandedSources.remove(row.source)
                    }
                }
            } label: {
                HStack(alignment: .firstTextBaseline, spacing: Spacing.sm) {
                    RoundedRectangle(cornerRadius: CornerRadius.sm)
                        .fill(color(for: row.source))
                        .frame(width: 10, height: 10)

                    if canExpand {
                        Image(systemName: isSourceExpanded ? "chevron.down" : "chevron.right")
                            .font(Typography.caption(10))
                            .foregroundStyle(palette.textSecondary)
                    }

                    Text(row.label)
                        .font(Typography.caption())
                        .foregroundStyle(palette.text)

                    Spacer(minLength: 0)

                    Text(formatTokenCount(row.tokens))
                        .font(Typography.caption())
                        .foregroundStyle(palette.textSecondary)
                        .monospacedDigit()
                }
                .frame(maxWidth: .infinity, alignment: .leading)
            }
            .buttonStyle(.plain)
            .accessibilityLabel("\(row.label) context source")

            if let metadata = row.metadata, !metadata.isEmpty {
                Text(metadata)
                    .font(Typography.caption(11))
                    .foregroundStyle(palette.textSecondary)
                    .padding(.leading, Spacing.lg + Spacing.xs)
            }

            if isSourceExpanded && canExpand {
                LazyVStack(alignment: .leading, spacing: Spacing.xs) {
                    ForEach(Array(documents.visible.enumerated()), id: \.offset) { entry in
                        let document = entry.element
                        HStack(alignment: .top, spacing: Spacing.sm) {
                            Text(document.path)
                                .font(Typography.caption(11))
                                .foregroundStyle(palette.textSecondary)
                                .lineLimit(1)
                                .truncationMode(.middle)

                            Spacer(minLength: 0)

                            Text(formatTokenCount(document.tokens))
                                .font(Typography.caption(11))
                                .foregroundStyle(palette.textSecondary)
                                .monospacedDigit()
                        }
                    }

                    if documents.remainingCount > 0 {
                        Text("…\(documents.remainingCount) more")
                            .font(Typography.caption(11))
                            .foregroundStyle(palette.textSecondary)
                    }
                }
                .padding(.leading, Spacing.lg + Spacing.xs)
            }
        }
        .padding(.vertical, Spacing.xxs)
    }

    private func color(for source: String) -> Color {
        switch source {
        case "step":
            return palette.accent
        case "direction":
            return .statusWarning
        case "diff":
            return .statusInfo
        case "repo_doc":
            return .statusSuccess
        case "scratch":
            return .statusWarning.opacity(0.6)
        case "wave":
            return palette.accent.opacity(0.6)
        case "wave_memory":
            return palette.textSecondary
        case "summary":
            return palette.textSecondary
        case "area":
            return .statusSuccess.opacity(0.7)
        case "clipboard":
            return .statusError
        default:
            return palette.textSecondary
        }
    }
}

struct ContextSourceRow: Equatable {
    let source: String
    let label: String
    let tokens: UInt64
    let metadata: String?
}

struct ContextDocumentSlice: Equatable {
    let visible: [DocumentEntry]
    let remainingCount: Int
}

func contextSourceRows(snapshot: ContextSnapshot) -> [ContextSourceRow] {
    snapshot.sources
        .filter { $0.value > 0 }
        .map { source, tokens in
            ContextSourceRow(
                source: source,
                label: contextSourceLabel(source),
                tokens: tokens,
                metadata: contextSourceMetadata(snapshot: snapshot, source: source)
            )
        }
        .sorted { lhs, rhs in
            if lhs.tokens == rhs.tokens {
                return contextSourceSortIndex(lhs.source) < contextSourceSortIndex(rhs.source)
            }
            return lhs.tokens > rhs.tokens
        }
}

func contextDocumentSlice(
    snapshot: ContextSnapshot,
    source: String,
    limit: Int = 10
) -> ContextDocumentSlice {
    let documents = snapshot.documents
        .filter { $0.source == source }
        .sorted { lhs, rhs in
            if lhs.tokens == rhs.tokens {
                return lhs.path < rhs.path
            }
            return lhs.tokens > rhs.tokens
        }

    let visible = Array(documents.prefix(limit))
    return ContextDocumentSlice(
        visible: visible,
        remainingCount: max(documents.count - visible.count, 0)
    )
}

func contextSourceMetadata(snapshot: ContextSnapshot, source: String) -> String? {
    let fileCount = snapshot.sourceCounts[source] ?? 0
    switch source {
    case "step":
        return snapshot.stepName
    case "direction":
        return snapshot.directionNames.isEmpty ? nil : snapshot.directionNames.joined(separator: ", ")
    case "system":
        return "loopflow"
    case "diff":
        let tier = contextDiffTierLabel(snapshot.diffTier)
        if fileCount > 0 {
            if let tier {
                return "\(tier) (\(fileCountLabel(fileCount)))"
            }
            return fileCountLabel(fileCount)
        }
        return tier
    case "repo_doc", "scratch", "summary":
        return fileCount > 0 ? fileCountLabel(fileCount) : nil
    case "wave":
        var details: [String] = []
        if let waveName = snapshot.waveName, !waveName.isEmpty {
            details.append(waveName)
        }
        if fileCount > 0 {
            details.append(fileCountLabel(fileCount))
        }
        return details.isEmpty ? nil : details.joined(separator: " · ")
    case "wave_memory":
        return fileCount > 0 ? fileCountLabel(fileCount) : "wave"
    case "area":
        if let areaName = snapshot.areaName, !areaName.isEmpty {
            if fileCount > 0 {
                return "\(areaName) (\(fileCountLabel(fileCount)))"
            }
            return areaName
        }
        return fileCount > 0 ? fileCountLabel(fileCount) : nil
    case "clipboard":
        return snapshot.hasClipboard ? "pasted" : nil
    default:
        return fileCount > 0 ? fileCountLabel(fileCount) : nil
    }
}

func contextSourceLabel(_ source: String) -> String {
    switch source {
    case "repo_doc":
        return "docs"
    case "wave_memory":
        return "memory"
    default:
        return source.replacingOccurrences(of: "_", with: " ")
    }
}

func contextSourceSortIndex(_ source: String) -> Int {
    let order = [
        "step",
        "direction",
        "system",
        "diff",
        "repo_doc",
        "scratch",
        "area",
        "wave",
        "wave_memory",
        "summary",
        "clipboard",
    ]
    return order.firstIndex(of: source) ?? Int.max
}

func contextDiffTierLabel(_ rawValue: String) -> String? {
    switch rawValue {
    case "UnifiedDiff":
        return "unified"
    case "StatOnly":
        return "stat"
    default:
        return nil
    }
}

func fileCountLabel(_ count: UInt64) -> String {
    let noun = count == 1 ? "file" : "files"
    return "\(count.formatted(.number.grouping(.automatic))) \(noun)"
}

func formatTokenCount(_ tokens: UInt64) -> String {
    tokens.formatted(.number.grouping(.automatic))
}

func formatContextBudget(_ budget: UInt64) -> String {
    if budget >= 1_000, budget % 1_000 == 0 {
        return "\(budget / 1_000)k"
    }
    return budget.formatted(.number.grouping(.automatic))
}

private struct ContextSegment: Identifiable {
    let source: String
    let fraction: Double
    let color: Color

    var id: String { source }
}
