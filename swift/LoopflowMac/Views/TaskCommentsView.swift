import Foundation
import Loopflow
import SwiftUI

/// Collapsed Comments below a Task's Description. The count and thread come
/// from the selected Task's own Linear read; a failed read keeps any earlier
/// thread visible and says it may be out of date. Comments are never derived
/// from the Description.
struct TaskCommentsView: View {
    let model: PodiumModel
    let task: RoadmapTask
    let wave: WaveSnapshot
    @Environment(\.palette) private var palette

    private var reading: PodiumReading<TaskComments> { model.comments(for: task.id) }
    private var thread: TaskComments? { reading.value }
    private var expanded: Bool { model.navigation.expandedComments.contains(task.id) }
    private var inFlight: Bool { model.commentsInFlight.contains(task.id) }

    var body: some View {
        VStack(alignment: .leading, spacing: Spacing.sm) {
            HStack(spacing: Spacing.sm) {
                Button {
                    let navigation = model.navigation
                    if navigation.expandedComments.remove(task.id) == nil {
                        navigation.expandedComments.insert(task.id)
                        Task { await model.loadComments(task: task, wave: wave) }
                    }
                } label: {
                    HStack(spacing: Spacing.xs) {
                        Image(systemName: expanded ? "chevron.down" : "chevron.right")
                            .font(.system(size: 10, weight: .semibold))
                        Text(heading)
                            .font(Typography.caption(11).weight(.semibold))
                            .tracking(0.8)
                            .textCase(.uppercase)
                    }
                    .foregroundStyle(palette.textSecondary)
                    .contentShape(Rectangle())
                }
                .buttonStyle(.plain)
                .accessibilityLabel(accessibilityHeading)
                .accessibilityIdentifier("task-comments-toggle")
                if inFlight {
                    ProgressView().controlSize(.mini)
                } else if reading.errorMessage != nil {
                    Text(thread == nil ? "Unavailable" : "May be out of date")
                        .font(Typography.caption(11))
                        .foregroundStyle(Color.statusWarning)
                        .accessibilityIdentifier("task-comments-stale")
                    Button("Retry") {
                        Task { await model.loadComments(task: task, wave: wave) }
                    }
                    .buttonStyle(.link)
                    .font(Typography.caption(11))
                    .accessibilityIdentifier("task-comments-retry")
                }
            }
            .padding(.top, Spacing.sm)
            if expanded {
                if let thread { threadView(thread) } else { statusText }
            }
        }
        .task(id: task.id) { await model.loadComments(task: task, wave: wave) }
    }

    private var heading: String {
        guard let thread else { return "Comments" }
        return "Comments (\(thread.comments.count))"
    }

    private var accessibilityHeading: String {
        let state = expanded ? "expanded" : "collapsed"
        guard let thread else { return "Comments, count not yet read, \(state)" }
        return "Comments, \(thread.comments.count), \(state)"
    }

    @ViewBuilder
    private var statusText: some View {
        Text(reading.errorMessage.map { "Comments could not be read: \($0)" } ?? "Reading comments…")
            .font(Typography.body(12))
            .foregroundStyle(palette.textSecondary)
            .textSelection(.enabled)
            .accessibilityIdentifier("task-comments-status")
    }

    private func threadView(_ thread: TaskComments) -> some View {
        VStack(alignment: .leading, spacing: Spacing.md) {
            if let reason = reading.errorMessage {
                Text("Latest read failed: \(reason)")
                    .font(Typography.body(12))
                    .foregroundStyle(palette.textSecondary)
                    .textSelection(.enabled)
                    .accessibilityIdentifier("task-comments-status")
            }
            if thread.comments.isEmpty {
                Text("No comments.")
                    .font(Typography.body(12))
                    .foregroundStyle(palette.textSecondary)
                    .accessibilityIdentifier("task-comments-empty")
            }
            ForEach(thread.comments) { comment in
                VStack(alignment: .leading, spacing: Spacing.xs) {
                    HStack(spacing: Spacing.sm) {
                        Text(Self.author(comment.author))
                            .font(.system(size: 12, weight: .semibold))
                            .foregroundStyle(palette.text)
                        Text(Self.date(comment.createdAt))
                            .font(Typography.caption(11))
                            .foregroundStyle(palette.textSecondary)
                    }
                    let body = Self.readableBody(comment.body)
                    if body.isEmpty {
                        Text("Empty comment").font(Typography.body(12)).foregroundStyle(palette.textSecondary)
                    } else {
                        MarkdownBlocks(source: body)
                    }
                }
                .padding(.leading, Spacing.sm)
                .overlay(alignment: .leading) { Rectangle().fill(palette.border).frame(width: 2) }
                .accessibilityIdentifier("task-comment-\(comment.id)")
            }
        }
        .accessibilityIdentifier("task-comments-thread")
    }

    static func author(_ author: TaskCommentAuthor) -> String {
        switch author {
        case .person(let name): name ?? "Unnamed person"
        case .integration: "Integration"
        }
    }

    static func date(_ raw: String?) -> String {
        guard let raw else { return "Date unavailable" }
        let parser = ISO8601DateFormatter()
        parser.formatOptions = [.withInternetDateTime, .withFractionalSeconds]
        let date = parser.date(from: raw) ?? {
            parser.formatOptions = [.withInternetDateTime]
            return parser.date(from: raw)
        }()
        guard let date else { return raw }
        return date.formatted(date: .abbreviated, time: .shortened)
    }

    /// Loopflow's machine markers are HTML comments; the stored body keeps them.
    static func readableBody(_ body: String) -> String {
        body.replacingOccurrences(of: "<!--[\\s\\S]*?-->", with: "", options: .regularExpression)
            .trimmingCharacters(in: .whitespacesAndNewlines)
    }
}
