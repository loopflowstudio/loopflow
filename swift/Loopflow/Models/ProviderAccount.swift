import Foundation

/// `lf account [provider] --json` as setup reads it. Rust owns the full
/// report; Swift keeps each row's identity and the provider CLI's own reading.
public struct ProviderAccountReport: Codable, Equatable, Sendable {
    public let accounts: [ProviderAccountRow]

    /// The ordinary login on this Mac, as the provider's CLI reported it.
    public func local(_ provider: String) -> ProviderAccountRow? {
        accounts.first { $0.provider == provider && $0.scope == .local && $0.executable != nil }
    }
}

public struct ProviderAccountRow: Codable, Equatable, Sendable {
    public enum Scope: String, Codable, Sendable {
        case managed, local
    }

    public enum Executable: String, Codable, Sendable {
        case installed, missing
    }

    public enum Verification: String, Codable, Sendable {
        case accepted, rejected, unavailable
        case notChecked = "not_checked"
    }

    /// Where a guided setup step stands. Unavailable is never signed out.
    public enum SetupState: Equatable, Sendable {
        case missing
        case signedOut
        case connected(login: String?)
        case unavailable(reason: String?)
    }

    public let provider: String
    public let login: String?
    public let scope: Scope
    public let source: String
    /// Null for managed rows, providers without a native CLI and cached reads.
    public let executable: Executable?
    public let verification: Verification
    public let diagnostic: String?
    /// The `lf` command that moves this row forward, when one does.
    public let recovery: String?

    public var setupState: SetupState {
        switch (executable, verification) {
        case (.missing, _): .missing
        case (_, .accepted): .connected(login: login)
        case (_, .rejected): .signedOut
        default: .unavailable(reason: diagnostic)
        }
    }
}
