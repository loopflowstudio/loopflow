import Foundation
import Observation
import SwiftUI

/// The one line that tells a person about something they did, or a read that
/// failed: at most one action, gone after six seconds unless it is a condition
/// (the Machine is away), which stays until it ends.
public struct Notice: Identifiable {
    public enum Kind: Sendable { case event, condition }

    public struct Action {
        public let title: String
        public let help: String?
        public let perform: @MainActor () -> Void

        public init(_ title: String, help: String? = nil, perform: @escaping @MainActor () -> Void) {
            self.title = title
            self.help = help
            self.perform = perform
        }
    }

    public let id = UUID()
    /// Posting a Notice with the key of the one shown refreshes it instead of
    /// replacing it: one Notice per failing part, not one per failure.
    public let key: String
    public let text: String
    public let action: Action?
    public let kind: Kind

    public init(_ text: String, key: String? = nil, action: Action? = nil, kind: Kind = .event) {
        self.key = key ?? text
        self.text = text
        self.action = action
        self.kind = kind
    }
}

/// One window's (Desktop) or app's (phone) Notice: at most one event over the
/// standing conditions. The event shows, else the newest condition; when the
/// event expires the condition shows again.
@MainActor
@Observable
public final class NoticeCenter {
    public static let eventLifetime = Duration.seconds(6)

    public private(set) var event: Notice?
    /// Standing conditions, oldest first; one per key.
    public private(set) var conditions: [Notice] = []
    /// Moves on every event posted, so a refreshed event restarts its six seconds.
    public private(set) var posted = 0

    public init() {}

    public var current: Notice? { event ?? conditions.last }

    /// A new event replaces the current one; a condition replaces the one with its key.
    public func post(_ notice: Notice) {
        switch notice.kind {
        case .event:
            event = notice
            posted += 1
        case .condition:
            if let index = conditions.firstIndex(where: { $0.key == notice.key }) {
                // The same condition read again stays as shown.
                if conditions[index].text != notice.text { conditions[index] = notice }
            } else {
                conditions.append(notice)
            }
        }
    }

    /// End the Notice keyed `key`: its condition is over, or its event no longer holds.
    public func clear(key: String) {
        conditions.removeAll { $0.key == key }
        if event?.key == key { event = nil }
    }

    /// Every reading an app shows reports here, by its failure. One turning into a
    /// failure posts one event keyed by its part, and its return clears it. A lost
    /// link posts nothing: the away condition says it once. A part whose failure
    /// was held back then posts its first failing reading once the link is back.
    public func read(_ part: String, _ what: String, from previous: String?, to next: String?,
                     link: PendingChanges.Link, action: Notice.Action? = nil) {
        let key = "read-\(part)"
        guard let reason = next else {
            heldFailures.remove(part)
            if previous != nil { clear(key: key) }
            return
        }
        guard previous == nil || heldFailures.contains(part) else { return }
        guard link == .connected else { heldFailures.insert(part); return }
        heldFailures.remove(part)
        post(Notice("\(what) couldn't be read: \(reason)", key: key, action: action))
    }

    /// Parts whose failure began while the link was lost and has not been posted.
    @ObservationIgnored private var heldFailures: Set<String> = []

    /// Put away what is shown: the event, else the newest condition.
    public func dismiss() {
        if event != nil {
            event = nil
        } else if !conditions.isEmpty {
            conditions.removeLast()
        }
    }

    /// What the overlay runs after showing an event for its lifetime.
    func expire(_ posted: Int) {
        guard posted == self.posted else { return }
        event = nil
    }
}

/// The overlay at the bottom of a window: one line, at most one action.
public struct NoticeView: View {
    let center: NoticeCenter
    @Environment(\.palette) private var palette

    public init(center: NoticeCenter) { self.center = center }

    public var body: some View {
        ZStack {
            if let notice = center.current {
                HStack(spacing: Spacing.md) {
                    Text(notice.text)
                        .lineLimit(1)
                        .accessibilityIdentifier("notice")
                    if let action = notice.action {
                        Button(action.title) {
                            center.dismiss()
                            action.perform()
                        }
                        .buttonStyle(.plain)
                        .fontWeight(.semibold)
                        .foregroundStyle(palette.success)
                        .help(action.help ?? action.title)
                        .accessibilityIdentifier("notice-action")
                    }
                }
                .font(Typography.text)
                .foregroundStyle(palette.text)
                .padding(.leading, Spacing.lg)
                .padding(.trailing, Spacing.md)
                .padding(.vertical, Spacing.sm)
                .background(RoundedRectangle(cornerRadius: CornerRadius.row).fill(palette.surfaceElement))
                .overlay(RoundedRectangle(cornerRadius: CornerRadius.row).strokeBorder(palette.border, lineWidth: 1))
                .shadow(color: palette.shadow, radius: 8, y: 2)
                .padding(.bottom, Spacing.xl)
                .transition(.move(edge: .bottom).combined(with: .opacity))
            }
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .bottom)
        .animation(.easeOut(duration: 0.18), value: center.current?.id)
        .task(id: center.posted) {
            let posted = center.posted
            try? await Task.sleep(for: NoticeCenter.eventLifetime)
            if !Task.isCancelled { center.expire(posted) }
        }
    }
}
