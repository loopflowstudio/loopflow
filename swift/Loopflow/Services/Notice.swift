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

/// One window's (Desktop) or app's (phone) Notice. A new Notice replaces the last.
@MainActor
@Observable
public final class NoticeCenter {
    public static let eventLifetime = Duration.seconds(6)

    public private(set) var current: Notice?
    /// Moves on every post, so a refreshed Notice restarts its six seconds.
    public private(set) var posted = 0

    public init() {}

    public func post(_ notice: Notice) {
        current = notice
        posted += 1
    }

    /// End the Notice keyed `key`: its condition is over.
    public func clear(key: String) {
        if current?.key == key { current = nil }
    }

    public func dismiss() { current = nil }

    /// What the overlay runs after showing an event for its lifetime.
    func expire(_ posted: Int) {
        guard posted == self.posted, current?.kind == .event else { return }
        current = nil
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
                .font(Typography.body(12))
                .foregroundStyle(palette.text)
                .padding(.leading, 14)
                .padding(.trailing, 10)
                .padding(.vertical, 8)
                .background(RoundedRectangle(cornerRadius: 9).fill(palette.surfaceElement))
                .overlay(RoundedRectangle(cornerRadius: 9).strokeBorder(palette.border, lineWidth: 1))
                .shadow(color: palette.shadow, radius: 8, y: 2)
                .padding(.bottom, 18)
                .transition(.move(edge: .bottom).combined(with: .opacity))
            }
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .bottom)
        .animation(.easeOut(duration: 0.18), value: center.posted)
        .task(id: center.posted) {
            let posted = center.posted
            try? await Task.sleep(for: NoticeCenter.eventLifetime)
            if !Task.isCancelled { center.expire(posted) }
        }
    }
}
