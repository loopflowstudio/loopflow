import Foundation
import Loopflow
import SwiftUI

struct ControlRoomActivityStrip: View {
    @Bindable var model: ControlRoomModel
    @Environment(\.palette) private var palette

    var body: some View {
        Group {
            if let snapshot = model.activity.value {
                available(snapshot)
            } else if let reason = model.activity.errorMessage {
                unavailable(reason)
            } else {
                loading
            }
        }
        .frame(maxWidth: .infinity, minHeight: 58, alignment: .leading)
        .background(palette.surface)
        .accessibilityElement(children: .contain)
        .accessibilityLabel("Live Loopflow activity")
        .accessibilityIdentifier("control-room-activity")
    }

    private func available(_ snapshot: ActivitySnapshot) -> some View {
        let summary = snapshot.controlRoomSummary
        return HStack(spacing: Spacing.xl) {
            VStack(alignment: .leading, spacing: Spacing.xxs) {
                Text("LIVE ACTIVITY")
                    .font(Typography.caption(9).weight(.bold))
                    .tracking(1.2)
                    .foregroundStyle(palette.textSecondary)
                Text("Observed \(observedTime(snapshot))")
                    .font(Typography.caption(10))
                    .foregroundStyle(palette.textSecondary)
                    .accessibilityLabel("Evidence observed \(observedTime(snapshot))")
            }

            metric("Agents", value: summary.activeAgents.formatted())
            metric("Motion", value: motion(summary))
            metric(
                "5m output",
                value: "\(formatRate(summary.outputTokensPerSecond5m)) tok/s"
            )
            metric("Measured", value: "\(compactTokens(summary.measuredOutputTokens)) tokens")

            Spacer(minLength: 0)

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
                evidenceBadge(
                    "Refresh failed",
                    systemImage: "arrow.clockwise.circle.fill",
                    color: .statusWarning
                )
                .help(reason)
                .accessibilityHint(reason)
            }
        }
        .padding(.horizontal, Spacing.lg)
        .padding(.vertical, Spacing.sm)
    }

    private var loading: some View {
        HStack(spacing: Spacing.sm) {
            ProgressView()
                .controlSize(.small)
            Text("Reading live activity…")
                .font(Typography.body(11))
                .foregroundStyle(palette.textSecondary)
        }
        .padding(.horizontal, Spacing.lg)
        .accessibilityIdentifier("control-room-activity-loading")
    }

    private func unavailable(_ reason: String) -> some View {
        HStack(spacing: Spacing.sm) {
            Image(systemName: "exclamationmark.triangle.fill")
                .foregroundStyle(Color.statusWarning)
            VStack(alignment: .leading, spacing: Spacing.xxs) {
                Text("Live activity unavailable")
                    .font(Typography.body(11).weight(.semibold))
                    .foregroundStyle(palette.text)
                Text(reason)
                    .font(Typography.caption(10))
                    .foregroundStyle(palette.textSecondary)
                    .lineLimit(1)
            }
        }
        .padding(.horizontal, Spacing.lg)
        .accessibilityIdentifier("control-room-activity-unavailable")
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
