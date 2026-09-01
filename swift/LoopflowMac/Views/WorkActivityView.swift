import Loopflow
import SwiftUI

struct WorkActivityView: View {
    @Bindable var model: PodiumModel

    @Environment(\.palette) private var palette
    @State private var isSettingTurnIntent = false
    @State private var turnIntentError: String?

    var body: some View {
        VStack(spacing: 0) {
            header
            Divider()
            if let reason = model.workActivity.errorMessage {
                evidenceBanner(reason)
            }
            if let turnIntentError {
                evidenceBanner(turnIntentError)
            }
            content
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
        .background(palette.surface)
        .accessibilityElement(children: .contain)
        .accessibilityIdentifier("podium-activity")
        .task(id: model.selection) {
            await model.refreshWorkActivity()
        }
    }

    private var header: some View {
        VStack(alignment: .leading, spacing: Spacing.sm) {
            HStack(alignment: .firstTextBaseline, spacing: Spacing.sm) {
                VStack(alignment: .leading, spacing: Spacing.xxs) {
                    Text("Activity")
                        .font(Typography.sectionTitle(20))
                        .foregroundStyle(palette.text)
                    Text(scopeTitle)
                        .font(Typography.caption(10))
                        .foregroundStyle(palette.textSecondary)
                        .lineLimit(1)
                        .accessibilityIdentifier("podium-activity-scope")
                }
                Spacer(minLength: Spacing.sm)
                if let wave = selectedWave, model.selection?.kind == .wave {
                    Button(wave.paused ? "Resume" : "Pause") {
                        Task { await setPaused(!wave.paused, waveId: wave.id) }
                    }
                    .buttonStyle(.borderless)
                    .font(Typography.caption(9).weight(.semibold))
                    .disabled(isSettingTurnIntent)
                    .accessibilityIdentifier("podium-wave-turn-control")
                }
                if model.selection != nil {
                    Button {
                        model.select(nil)
                    } label: {
                        Image(systemName: "xmark")
                    }
                    .buttonStyle(.borderless)
                    .help("Show all Activity")
                    .accessibilityLabel("Show all Activity")
                }
            }

            if let selectedSummary {
                Text(selectedSummary)
                    .font(Typography.body(11))
                    .foregroundStyle(palette.text)
                    .lineLimit(2)
                    .textSelection(.enabled)
                    .accessibilityIdentifier("podium-selection-summary")
            }
        }
        .padding(.horizontal, Spacing.lg)
        .padding(.vertical, Spacing.md)
    }

    @ViewBuilder
    private var content: some View {
        if model.workActivity.value == nil, model.workActivity.errorMessage == nil {
            ProgressView("Reading Activity…")
                .frame(maxWidth: .infinity, maxHeight: .infinity)
                .accessibilityIdentifier("podium-activity-loading")
        } else if let items = model.workActivity.value?.items, !items.isEmpty {
            ScrollView {
                LazyVStack(alignment: .leading, spacing: 0) {
                    ForEach(items) { entry in
                        activityRow(entry)
                        Divider()
                    }
                    if model.workActivity.value?.truncated == true {
                        Text("Older Activity exists outside this window.")
                            .font(Typography.caption(9))
                            .foregroundStyle(palette.textSecondary)
                            .padding(Spacing.lg)
                    }
                }
            }
            .accessibilityIdentifier("podium-activity-list")
        } else {
            ContentUnavailableView(
                "No Activity in this window",
                systemImage: "clock.arrow.circlepath",
                description: Text("Work creation, Runs, PRs, and Steers will appear here.")
            )
            .accessibilityIdentifier("podium-activity-empty")
        }
    }

    private func activityRow(_ entry: WorkActivityEntry) -> some View {
        let appearance = activityAppearance(entry.fact)
        return HStack(alignment: .top, spacing: Spacing.md) {
            ZStack {
                Circle()
                    .fill(appearance.color.opacity(0.12))
                    .frame(width: 30, height: 30)
                Image(systemName: appearance.icon)
                    .font(.system(size: 11, weight: .semibold))
                    .foregroundStyle(appearance.color)
            }

            VStack(alignment: .leading, spacing: Spacing.xs) {
                HStack(alignment: .firstTextBaseline, spacing: Spacing.xs) {
                    Text("\(entry.work.kind.rawValue.uppercased()) · \(entry.subject)")
                        .font(Typography.caption(8).weight(.bold))
                        .tracking(0.6)
                        .foregroundStyle(palette.textSecondary)
                        .lineLimit(1)
                    Spacer(minLength: 0)
                    Text(activityTime(entry.recordedAt))
                        .font(Typography.code(8))
                        .foregroundStyle(palette.textSecondary)
                        .help(exactTime(entry.recordedAt))
                }
                Text(entry.summary)
                    .font(Typography.body(11).weight(.semibold))
                    .foregroundStyle(palette.text)
                    .fixedSize(horizontal: false, vertical: true)
                    .textSelection(.enabled)

                if let github = entry.fact.github {
                    Link("Open PR #\(github.number)", destination: github.url)
                        .font(Typography.caption(9).weight(.semibold))
                        .foregroundStyle(Color.loopflowBurgundy)
                        .accessibilityIdentifier("podium-open-pr-\(github.number)")
                }
            }
        }
        .padding(.horizontal, Spacing.lg)
        .padding(.vertical, Spacing.md)
        .frame(maxWidth: .infinity, alignment: .leading)
        .accessibilityElement(children: .contain)
        .accessibilityLabel("\(entry.subject), \(entry.summary), \(exactTime(entry.recordedAt))")
        .accessibilityIdentifier("podium-activity-\(entry.id)")
    }

    private var selectedWave: WaveSnapshot? {
        guard let selection = model.selection else { return nil }
        guard let waveId = model.waveId(for: selection) else { return nil }
        return model.wave(id: waveId)?.wave
    }

    private var scopeTitle: String {
        switch model.selection {
        case nil:
            "All durable Work"
        case let work?:
            switch work.kind {
            case .wave:
                "Wave · \(waveName(work.id))"
            case .project:
                "Project · \(model.project(id: work.id)?.project.project.name ?? work.id)"
            case .task:
                "Task · \(model.task(id: work.id)?.task.task.identifier ?? work.id)"
            }
        }
    }

    private var selectedSummary: String? {
        switch model.selection {
        case nil:
            nil
        case let work?:
            switch work.kind {
            case .wave:
                model.wave(id: work.id)?.wave.goal
            case .project:
                model.project(id: work.id)?.project.project.definition
            case .task:
                model.task(id: work.id)?.task.condition.reason
            }
        }
    }

    private func waveName(_ waveId: String) -> String {
        model.wave(id: waveId)?.wave.name
            ?? model.rosterWave(id: waveId)?.displayName
            ?? "Wave"
    }

    private func activityAppearance(_ fact: WorkActivityFact) -> (icon: String, color: Color) {
        switch fact {
        case .workCreated: ("plus", .statusNeutral)
        case .runStarted: ("play.fill", .statusInfo)
        case .runFinished(_, let status):
            (
                "checkmark",
                ["ok", "completed", "succeeded"].contains(status.lowercased())
                    ? .statusSuccess
                    : .statusWarning
            )
        case .prStarted: ("arrow.triangle.branch", .statusNeutral)
        case .prPublishRequested: ("arrow.up.right", .statusInfo)
        case .prMergeRequested: ("arrow.triangle.merge", .statusInfo)
        case .prMerged: ("checkmark.seal.fill", .statusSuccess)
        case .prAbandoned: ("xmark", .statusWarning)
        case .steerIssued: ("arrow.turn.down.right", Color.loopflowBurgundy)
        }
    }

    private func activityTime(_ timestamp: Int64) -> String {
        Date(timeIntervalSince1970: TimeInterval(timestamp))
            .formatted(.relative(presentation: .named))
    }

    private func exactTime(_ timestamp: Int64) -> String {
        Date(timeIntervalSince1970: TimeInterval(timestamp))
            .formatted(date: .abbreviated, time: .shortened)
    }

    private func evidenceBanner(_ reason: String) -> some View {
        Label(reason, systemImage: "exclamationmark.triangle.fill")
            .font(Typography.caption(9))
            .foregroundStyle(Color.statusWarning)
            .lineLimit(2)
            .textSelection(.enabled)
            .padding(.horizontal, Spacing.lg)
            .padding(.vertical, Spacing.sm)
            .frame(maxWidth: .infinity, alignment: .leading)
            .background(Color.statusWarning.opacity(0.10))
    }

    @MainActor
    private func setPaused(_ paused: Bool, waveId: String) async {
        isSettingTurnIntent = true
        turnIntentError = nil
        defer { isSettingTurnIntent = false }
        do {
            try await model.setWavePaused(waveId: waveId, paused: paused)
        } catch {
            turnIntentError = error.localizedDescription
        }
    }
}
