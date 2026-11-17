import Foundation

/// The Machine's last answer to one subscription, with the revisions it was read at.
/// While reads fail, `value` is the last good answer and `failure` says why.
public struct Reading<Value> {
    public let value: Value?
    /// The revisions `value` was read at; nil before a reading or without a store.
    public let readAt: Revisions?
    public let failure: String?

    public static var loading: Reading { Reading(value: nil, readAt: nil, failure: nil) }

    public static func available(_ value: Value, readAt: Revisions? = nil) -> Reading {
        Reading(value: value, readAt: readAt, failure: nil)
    }

    public static func unavailable(lastGood: Value?, reason: String, readAt: Revisions? = nil) -> Reading {
        Reading(value: lastGood, readAt: readAt, failure: reason)
    }

    public var isLoading: Bool { value == nil && failure == nil }

    /// The answer, when the last read succeeded.
    public var current: Value? { failure == nil ? value : nil }

    /// This reading's value kept as the last good one behind `reason`.
    public func failed(_ reason: String) -> Reading {
        .unavailable(lastGood: value, reason: reason, readAt: readAt)
    }

    public func map<Other>(_ transform: (Value) -> Other) -> Reading<Other> {
        Reading<Other>(value: value.map(transform), readAt: readAt, failure: failure)
    }

    /// Whether `next` may replace this reading: one read at a lower revision never does.
    public func yields(to next: Reading) -> Bool {
        guard let readAt, let later = next.readAt else { return true }
        return later.covers(readAt)
    }
}

extension Reading: Equatable where Value: Equatable {}
extension Reading: Sendable where Value: Sendable {}

/// Readings kept as wire bytes per Machine and subscription, so a launch draws
/// before the first read. The live decoder reads them back, so a file an older
/// app wrote fails to load instead of showing a stale shape.
public struct SavedReadings: Sendable {
    public let machine: String
    public let directory: URL

    public init(machine: String, directory: URL = .cachesDirectory) {
        self.machine = machine
        self.directory = directory
    }

    /// The reading saved for `subscription`; read it off the main thread where it can be large.
    public func load(_ subscription: String) -> Data? {
        try? Data(contentsOf: url(subscription))
    }

    public func save(_ wire: Data, _ subscription: String) {
        let url = url(subscription)
        Task.detached(priority: .utility) {
            #if os(iOS)
            try? wire.write(to: url, options: [.atomic, .completeFileProtectionUntilFirstUserAuthentication])
            #else
            try? wire.write(to: url, options: .atomic)
            #endif
        }
    }

    /// Removes every reading this Machine left here.
    public func forget() {
        let prefix = "readings-\(machine)-"
        let files = (try? FileManager.default.contentsOfDirectory(at: directory, includingPropertiesForKeys: nil)) ?? []
        for file in files where file.lastPathComponent.hasPrefix(prefix) {
            try? FileManager.default.removeItem(at: file)
        }
    }

    private func url(_ subscription: String) -> URL {
        let name = subscription.map { $0.isLetter || $0.isNumber || $0 == "-" || $0 == "_" ? $0 : "_" }
        return directory.appending(path: "readings-\(machine)-\(String(name)).json")
    }
}
