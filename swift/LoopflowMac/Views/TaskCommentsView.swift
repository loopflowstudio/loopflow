import Foundation
import Loopflow
import SwiftUI

/// Collapsed Comments below a Task's Description. The count and thread come
/// from the selected Task's planning reader; a failed read keeps any earlier
/// thread visible and says it may be out of date. Comments are never derived
/// from the Description.
struct TaskCommentsView: View {
    let model: WorkModel
    let task: RoadmapTask
    let wave: WaveSnapshot
    @Environment(\.palette) private var palette

    private var reading: WorkReading<TaskComments> { model.comments[task.id] }
    private var thread: TaskComments? { reading.value }
    private var expanded: Bool { model.navigation.expandedComments.contains(task.id) }
    private var inFlight: Bool { model.comments.inFlight.contains(task.id) }

    var body: some View {
        VStack(alignment: .leading, spacing: Spacing.sm) {
            WorkDisclosureHeading(
                title: "Comments",
                count: thread?.comments.count,
                expanded: expanded,
                inFlight: inFlight,
                failure: reading.errorMessage == nil ? nil : thread != nil,
                identifier: "task-comments",
                accessibilityLabel: accessibilityHeading,
                help: heading,
                toggle: {
                    let navigation = model.navigation
                    if navigation.expandedComments.remove(task.id) == nil {
                        navigation.expandedComments.insert(task.id)
                        Task { await model.loadComments(task: task, wave: wave) }
                    }
                },
                retry: { Task { await model.loadComments(task: task, wave: wave) } }
            )
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
            .font(Typography.body(12.5))
            .foregroundStyle(palette.textSecondary)
            .textSelection(.enabled)
            .padding(.leading, 17)
            .accessibilityIdentifier("task-comments-status")
    }

    private func threadView(_ thread: TaskComments) -> some View {
        VStack(alignment: .leading, spacing: 0) {
            if let reason = reading.errorMessage ?? thread.refreshError {
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
            ForEach(Array(thread.comments.enumerated()), id: \.element.id) { index, comment in
                VStack(alignment: .leading, spacing: 3) {
                    HStack(alignment: .firstTextBaseline, spacing: Spacing.sm) {
                        Text(Self.author(comment.author))
                            .font(Typography.strong(12.5))
                            .foregroundStyle(palette.text)
                        if thread.pendingSync.contains(comment.id) {
                            Text("Pending sync")
                                .font(Typography.caption(12))
                                .foregroundStyle(palette.textSecondary)
                        }
                        Text(Self.date(comment.createdAt))
                            .font(Typography.caption(12))
                            .foregroundStyle(palette.textTertiary)
                    }
                    if let conflict = thread.conflicts[comment.id] {
                        Text("Conflicting Linear comment")
                            .font(Typography.strong(12.5))
                        MarkdownBlocks(source: Self.readableBody(conflict)).equatable()
                    }
                    let body = Self.readableBody(comment.body)
                    if body.isEmpty {
                        Text("Empty comment").font(Typography.body(13)).foregroundStyle(palette.textSecondary)
                    } else {
                        MarkdownBlocks(source: body).equatable()
                    }
                }
                .padding(.vertical, Spacing.sm)
                .frame(maxWidth: .infinity, alignment: .leading)
                .overlay(alignment: .top) {
                    if index > 0 { Rectangle().fill(palette.border.opacity(0.8)).frame(height: 1) }
                }
                .accessibilityIdentifier("task-comment-\(comment.id)")
            }
        }
        .padding(.leading, 17)
        .frame(maxWidth: 680, alignment: .leading)
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
