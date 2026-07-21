import Foundation
import Loopflow
import SwiftUI

/// Machine-wide Wave health and agent motion, kept as independent readings.
/// At desktop widths both bands share one strip; narrower windows stack them so
/// no operational evidence is silently dropped to make the layout fit.
struct ControlRoomStatusStrip: View {
    @Bindable var model: ControlRoomModel
    @Environment(\.palette) private var palette

    var body: some View {
        ViewThatFits(in: .horizontal) {
            HStack(spacing: 0) {
                fleetBand
                Divider()
                    .frame(height: 34)
                    .padding(.horizontal, Spacing.lg)
                activityBand
            }
            .fixedSize(horizontal: true, vertical: false)
            .padding(.horizontal, Spacing.lg)
            .padding(.vertical, Spacing.sm)

            VStack(spacing: 0) {
                fleetBand
                    .padding(.horizontal, Spacing.lg)
                    .padding(.vertical, Spacing.sm)
                    .frame(maxWidth: .infinity, alignment: .leading)
                Divider()
                activityBand
                    .padding(.horizontal, Spacing.lg)
                    .padding(.vertical, Spacing.sm)
                    .frame(maxWidth: .infinity, alignment: .leading)
            }
        }
        .frame(maxWidth: .infinity, minHeight: 58, alignment: .leading)
        .background(palette.surface)
        .accessibilityElement(children: .contain)
        .accessibilityLabel("Loopflow fleet status")
        .accessibilityIdentifier("control-room-status")
    }

    @ViewBuilder
    private var fleetBand: some View {
        if let summary = model.fleetSummary {
            availableFleet(summary)
        } else if let reason = model.waves.errorMessage {
            unavailable(
                "Wave fleet unavailable",
                reason: reason,
                identifier: "control-room-fleet-unavailable"
            )
        } else {
            loading("Reading Wave fleet…", identifier: "control-room-fleet-loading")
        }
    }

    @ViewBuilder
    private var activityBand: some View {
        if let snapshot = model.activity.value {
            availableActivity(snapshot)
        } else if let reason = model.activity.errorMessage {
            unavailable(
                "Agent activity unavailable",
                reason: reason,
                identifier: "control-room-activity-unavailable"
            )
        } else {
            loading("Reading agent activity…", identifier: "control-room-activity-loading")
        }
    }

    private func availableFleet(_ summary: ControlRoomFleetSummary) -> some View {
        HStack(spacing: Spacing.xl) {
            bandTitle(
                "WAVE FLEET",
                detail: model.repoPath == nil ? "All repositories" : "Selected repository"
            )
            metric("Waves", value: summary.registeredWaves.formatted())
            metric("Runs", value: summary.activeRuns.formatted())
            metric("Listeners", value: summary.liveListeners.formatted())
            metric("Work", value: "\(summary.activeProjects)P · \(summary.activeTasks)T")

            if summary.unservedRuns > 0 {
                evidenceBadge(
                    summary.unservedRuns == 1
                        ? "1 Run · no listener"
                        : "\(summary.unservedRuns) Runs · no listeners",
                    systemImage: "waveform.slash",
                    color: .statusError
                )
            }
            if let reason = model.waves.errorMessage {
                refreshBadge(reason)
            }
        }
        .accessibilityElement(children: .contain)
        .accessibilityLabel("Wave fleet")
        .accessibilityIdentifier("control-room-fleet")
    }

    private func availableActivity(_ snapshot: ActivitySnapshot) -> some View {
        let summary = snapshot.controlRoomSummary
        return HStack(spacing: Spacing.xl) {
            bandTitle("AGENT ACTIVITY", detail: "Observed \(observedTime(snapshot))")
            metric("Agents", value: summary.activeAgents.formatted())
            metric("Motion", value: motion(summary))
            metric("5m output", value: "\(formatRate(summary.outputTokensPerSecond5m)) tok/s")
            metric("Measured", value: "\(compactTokens(summary.measuredOutputTokens)) tokens")

            if summary.orphaned > 0 {
                evidenceBadge(
                    "\(summary.orphaned) orphaned",
                    systemImage: "exclamationmark.triangle.fill",
                    color: .statusError
                )
            }
            if summary.unclaimed > 0 {
                evidenceBadge(
                    "\(summary.unclaimed) unclaimed",
                    systemImage: "questionmark.diamond.fill",
                    color: .statusWarning
                )
            }
            if let reason = model.activity.errorMessage {
                refreshBadge(reason)
            }
        }
        .accessibilityElement(children: .contain)
        .accessibilityLabel("Agent activity")
        .accessibilityIdentifier("control-room-activity")
    }

    private func bandTitle(_ title: String, detail: String) -> some View {
        VStack(alignment: .leading, spacing: Spacing.xxs) {
            Text(title)
                .font(Typography.caption(9).weight(.bold))
                .tracking(1.2)
                .foregroundStyle(palette.textSecondary)
            Text(detail)
                .font(Typography.caption(10))
                .foregroundStyle(palette.textSecondary)
        }
    }

    private func loading(_ label: String, identifier: String) -> some View {
        HStack(spacing: Spacing.sm) {
            ProgressView()
                .controlSize(.small)
            Text(label)
                .font(Typography.body(11))
                .foregroundStyle(palette.textSecondary)
        }
        .accessibilityIdentifier(identifier)
    }

    private func unavailable(_ title: String, reason: String, identifier: String) -> some View {
        HStack(spacing: Spacing.sm) {
            Image(systemName: "exclamationmark.triangle.fill")
                .foregroundStyle(Color.statusWarning)
            VStack(alignment: .leading, spacing: Spacing.xxs) {
                Text(title)
                    .font(Typography.body(11).weight(.semibold))
                    .foregroundStyle(palette.text)
                Text(reason)
                    .font(Typography.caption(10))
                    .foregroundStyle(palette.textSecondary)
                    .lineLimit(1)
            }
        }
        .accessibilityIdentifier(identifier)
    }

    private func metric(_ label: String, value: String) -> some View {
        VStack(alignment: .leading, spacing: Spacing.xxs) {
            Text(label.uppercased())
                .font(Typography.caption(8).weight(.bold))
                .tracking(0.8)
                .foregroundStyle(palette.textSecondary)
            Text(value)
                .font(Typography.code(11).weight(.medium))
                .foregroundStyle(palette.text)
                .lineLimit(1)
        }
        .accessibilityElement(children: .combine)
    }

    private func refreshBadge(_ reason: String) -> some View {
        evidenceBadge(
            "Refresh failed",
            systemImage: "arrow.clockwise.circle.fill",
            color: .statusWarning
        )
        .help(reason)
        .accessibilityHint(reason)
    }

    private func evidenceBadge(
        _ label: String,
        systemImage: String,
        color: Color
    ) -> some View {
        Label(label, systemImage: systemImage)
            .font(Typography.caption(9).weight(.semibold))
            .foregroundStyle(color)
            .lineLimit(1)
            .padding(.horizontal, Spacing.sm)
            .padding(.vertical, Spacing.xs)
            .background(color.opacity(0.1))
            .clipShape(Capsule())
    }

    private func motion(_ summary: ControlRoomActivitySummary) -> String {
        "\(summary.working) working · \(summary.waiting) waiting · \(summary.stalled) stalled"
    }

    private func formatRate(_ rate: Double) -> String {
        if rate >= 10 { return String(format: "%.0f", rate) }
        return String(format: "%.1f", rate)
    }

    private func observedTime(_ snapshot: ActivitySnapshot) -> String {
        Date(timeIntervalSince1970: TimeInterval(snapshot.observedAt))
            .formatted(date: .omitted, time: .shortened)
    }

    private func compactTokens(_ tokens: UInt64) -> String {
        switch tokens {
        case 1_000_000...:
            String(format: "%.1fM", Double(tokens) / 1_000_000)
        case 10_000...:
            "\(tokens / 1_000)k"
        case 1_000...:
            String(format: "%.1fk", Double(tokens) / 1_000)
        default:
            tokens.formatted()
        }
    }
}
