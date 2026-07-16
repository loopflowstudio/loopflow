import Foundation

// Typed references inside a Wave Chat message body. A message can name a Task
// (`W2-174`), a pull request (`PR #889`), and — reserved for later — a Project
// or a piece of evidence. Detection is a pure function over the message string;
// there is no new wire field or store. The Mac surface renders each detected
// span as an inline link with a compact popover instead of dead text.

/// What kind of object a detected reference points at. `project` and `evidence`
/// are reserved so the renderer's switch and any consumer stay total as detectors
/// grow; only `task` and `pullRequest` are detected today (the ones the product
/// contract's proof exercises).
public enum ChatReferenceKind: String, Sendable, Hashable, CaseIterable {
    case task
    case pullRequest
    case project
    case evidence
}

/// One detected reference: where it sits in the source string, what it points at,
/// its canonical identifier (`"W2-174"`, `"889"`), and the exact authored text so
/// the link keeps the sentence's own wording (`"PR #889"`, `"#889"`, `"W2-174"`).
public struct ChatReferenceMatch: Sendable, Hashable {
    public let range: Range<String.Index>
    public let kind: ChatReferenceKind
    public let identifier: String
    public let displayText: String

    public init(
        range: Range<String.Index>,
        kind: ChatReferenceKind,
        identifier: String,
        displayText: String
    ) {
        self.range = range
        self.kind = kind
        self.identifier = identifier
        self.displayText = displayText
    }
}

/// Detect typed references in a Chat message body, ordered by position and
/// non-overlapping. A Task reference is a Linear issue key; a PR reference is a
/// GitHub-style `#number`, optionally prefixed by `PR`.
public func parseChatReferences(in text: String) -> [ChatReferenceMatch] {
    // Cheap exit for the common case: nothing that could start a reference.
    guard text.contains("-") || text.contains("#") else { return [] }

    var matches: [ChatReferenceMatch] = []
    matches.append(contentsOf: taskReferences(in: text))
    matches.append(contentsOf: pullRequestReferences(in: text, excluding: matches))

    matches.sort { $0.range.lowerBound < $1.range.lowerBound }
    return matches
}

// MARK: - Task references (Linear issue keys)

// Team key (letter then up to four uppercase alphanumerics) + `-` + number, not
// glued to surrounding identifier characters. The denylist rejects the technical
// tokens that share this shape but never name a Task; it is a heuristic of the
// same kind Linear and GitHub apply when they linkify `ABC-123`.
private let taskKeyPattern = "(?<![A-Za-z0-9])([A-Z][A-Z0-9]{1,4})-([0-9]{1,6})(?![A-Za-z0-9])"

private let taskKeyDenylist: Set<String> = [
    "UTF", "SHA", "ISO", "ISBN", "RFC", "IPV", "COVID", "GPT",
]

private let taskKeyRegex = try? NSRegularExpression(pattern: taskKeyPattern)

private func taskReferences(in text: String) -> [ChatReferenceMatch] {
    guard let regex = taskKeyRegex else { return [] }
    let ns = text as NSString
    let full = NSRange(location: 0, length: ns.length)
    var results: [ChatReferenceMatch] = []
    for match in regex.matches(in: text, range: full) {
        guard let teamRange = Range(match.range(at: 1), in: text),
              let wholeRange = Range(match.range, in: text) else { continue }
        let team = String(text[teamRange])
        if taskKeyDenylist.contains(team) { continue }
        let identifier = ns.substring(with: match.range)
        results.append(ChatReferenceMatch(
            range: wholeRange,
            kind: .task,
            identifier: identifier,
            displayText: identifier
        ))
    }
    return results
}

// MARK: - Pull-request references

// `PR #889`, `PR#889`, or a bare `#889`. Every real form carries the `#`, which
// keeps the cheap `contains("#")` pre-check valid and avoids linkifying prose
// like "PR 889 files". The number is canonical; the display text keeps whatever
// the author wrote so the link reads naturally.
private let prPrefixedPattern = "(?<![A-Za-z0-9])PR\\s*#\\s*([0-9]{1,7})(?![A-Za-z0-9])"
private let bareHashPattern = "(?<![A-Za-z0-9#])#([0-9]{1,7})(?![A-Za-z0-9])"

private let prPrefixedRegex = try? NSRegularExpression(pattern: prPrefixedPattern)
private let bareHashRegex = try? NSRegularExpression(pattern: bareHashPattern)

private func pullRequestReferences(
    in text: String,
    excluding taken: [ChatReferenceMatch]
) -> [ChatReferenceMatch] {
    let ns = text as NSString
    let full = NSRange(location: 0, length: ns.length)
    var results: [ChatReferenceMatch] = []
    var claimed = taken.compactMap { NSRange($0.range, in: text) }

    func collect(_ regex: NSRegularExpression?) {
        guard let regex else { return }
        for match in regex.matches(in: text, range: full) {
            if claimed.contains(where: { NSIntersectionRange($0, match.range).length > 0 }) {
                continue
            }
            guard let wholeRange = Range(match.range, in: text) else { continue }
            let number = ns.substring(with: match.range(at: 1))
            results.append(ChatReferenceMatch(
                range: wholeRange,
                kind: .pullRequest,
                identifier: number,
                displayText: ns.substring(with: match.range)
            ))
            claimed.append(match.range)
        }
    }

    // `PR`-prefixed first so a bare-`#` pass can't split `PR #889` in two.
    collect(prPrefixedRegex)
    collect(bareHashRegex)
    return results
}

// MARK: - External targets

/// Build a GitHub pull-request URL from a resolved repository base and a PR
/// number. Returns `nil` when the base isn't a usable GitHub repository URL —
/// the caller then discloses the reference without an external link rather than
/// fabricating one.
public func githubPullRequestURL(base: URL?, number: String) -> URL? {
    guard let base, !number.isEmpty, number.allSatisfy(\.isNumber) else { return nil }
    return base.appendingPathComponent("pull").appendingPathComponent(number)
}

/// Normalize a git origin remote URL (`git@github.com:owner/repo.git`,
/// `https://github.com/owner/repo.git`, or `ssh://git@github.com/owner/repo`)
/// into `https://github.com/owner/repo`. Returns `nil` for non-GitHub or
/// unparseable remotes.
public func githubRepoBase(fromRemote remote: String) -> URL? {
    let trimmed = remote.trimmingCharacters(in: .whitespacesAndNewlines)
    guard !trimmed.isEmpty else { return nil }

    // Match `github.com` only at a real host boundary — after `@`, `/`, or the
    // string start — so a lookalike host such as `mygithub.com` isn't mistaken
    // for GitHub.
    var path: String
    if let range = trimmed.range(of: "github.com"),
       isHostBoundary(before: range.lowerBound, in: trimmed) {
        // Everything after the host, whether the separator was `:` (scp form)
        // or `/` (url form).
        path = String(trimmed[range.upperBound...])
        path = path.trimmingCharacters(in: CharacterSet(charactersIn: ":/"))
    } else {
        return nil
    }
    if path.hasSuffix(".git") { path = String(path.dropLast(4)) }
    path = path.trimmingCharacters(in: CharacterSet(charactersIn: "/"))

    let parts = path.split(separator: "/")
    guard parts.count >= 2 else { return nil }
    let owner = parts[0]
    let repo = parts[1]
    guard !owner.isEmpty, !repo.isEmpty else { return nil }
    return URL(string: "https://github.com/\(owner)/\(repo)")
}

private func isHostBoundary(before index: String.Index, in text: String) -> Bool {
    guard index > text.startIndex else { return true }
    let preceding = text[text.index(before: index)]
    return preceding == "@" || preceding == "/"
}
