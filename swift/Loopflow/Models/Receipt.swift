import Foundation

/// Which raw record a receipt points at. Wire tokens mirror Rust `EvidenceKind`.
public enum EvidenceKind: String, Codable, Sendable, Hashable {
    case chatTurn = "chat_turn"
    case workerReport = "worker_report"
    case trace
    case pm
    case pr
}

/// A stable pointer from a curated claim to the record that justifies it.
///
/// Mirrors Rust `Receipt` (`rust/loopflow/src/receipt.rs`): every field is
/// required, no defaults. A claim carries `[Receipt]`; one-to-one is the
/// degenerate case and many-to-one stays compact because each entry is an id,
/// not a copied record. `reference` is the canonical durable id for `kind`
/// (a `turn_id`, `run_id`, trace UUID, Linear issue UUID, or `owner/repo#N[@sha]`).
public struct Receipt: Codable, Sendable, Hashable {
    public let kind: EvidenceKind
    public let reference: String
    public let wave: String

    public init(kind: EvidenceKind, reference: String, wave: String) {
        self.kind = kind
        self.reference = reference
        self.wave = wave
    }

    /// The `kind:reference` drill token — the argument `lf receipt show` takes.
    public var token: String { "\(kind.rawValue):\(reference)" }

    /// A browser URL for a `pr:` receipt, or `nil` for kinds with no canonical
    /// web view. A PR reference is `owner/repo#N` optionally pinned with `@sha`;
    /// the link targets the pull request and drops the sha pin. Other kinds
    /// (chat turn, trace, worker report, PM) have no web address until the
    /// `lf receipt show` drill lands — their chips render the token only.
    public var browserURL: URL? {
        guard kind == .pr else { return nil }
        let unpinned = reference.split(separator: "@", maxSplits: 1).first.map(String.init) ?? reference
        guard let hash = unpinned.firstIndex(of: "#") else { return nil }
        let slug = unpinned[unpinned.startIndex..<hash]
        let number = unpinned[unpinned.index(after: hash)...]
        guard !slug.isEmpty, !number.isEmpty, number.allSatisfy(\.isNumber) else { return nil }
        return URL(string: "https://github.com/\(slug)/pull/\(number)")
    }
}

/// One curated memory fact with its evidence receipts. Mirrors Rust
/// `MemoryFact`, the wire shape of `lf memory log --json`.
public struct MemoryFact: Codable, Sendable, Hashable {
    public let fact: String
    public let receipts: [Receipt]

    public init(fact: String, receipts: [Receipt]) {
        self.fact = fact
        self.receipts = receipts
    }
}
