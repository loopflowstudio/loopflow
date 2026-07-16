import Foundation

// Typed references inside a Wave Chat message body. A message can name a Task
// (`W2-174`), a pull request (`PR #889`), a Project (`project:wave-chat`), or a
// piece of evidence (`evidence:run-abc123`). Task and PR use their natural
// syntax; Project and evidence use an explicit authored prefix because they have
// no unambiguous form in free prose. Detection is a pure function over the
// message string; there is no new wire field or store. The Mac surface renders
// each detected span as an inline link with a compact popover instead of dead
// text.

/// What kind of object a detected reference points at. All four kinds are
/// detected: `task` and `pullRequest` from their natural syntax, `project` and
/// `evidence` from an explicit `project:<slug>` / `evidence:<token>` prefix.
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
/// non-overlapping.
///
/// Two kinds have natural syntax and are detected unprefixed: a Task reference is
/// a Linear issue key (`W2-174`); a PR reference is a GitHub-style `#number`,
/// optionally prefixed by `PR`. Projects and evidence have no unambiguous form in
/// free prose, so they use the smallest authored contract — an explicit
/// `project:<slug>` or `evidence:<token>` — rather than being excluded.
public func parseChatReferences(in text: String) -> [ChatReferenceMatch] {
    // Cheap exit for the common case: nothing that could start a reference.
    guard text.contains("-") || text.contains("#")
        || text.contains("project:") || text.contains("evidence:") else { return [] }

    // Authored `project:` / `evidence:` refs are explicit, so they claim their
    // range first — an `evidence:W2-174` token is one evidence reference, not an
    // evidence reference wrapping a stray Task match.
    var matches: [ChatReferenceMatch] = authoredReferences(in: text)
    matches.append(contentsOf: taskReferences(in: text, excluding: matches))
    matches.append(contentsOf: pullRequestReferences(in: text, excluding: matches))

    matches.sort { $0.range.lowerBound < $1.range.lowerBound }
    return matches
}

private func overlaps(_ range: NSRange, _ claimed: [ChatReferenceMatch], in text: String) -> Bool {
    claimed.contains { NSIntersectionRange(NSRange($0.range, in: text), range).length > 0 }
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

private func taskReferences(
    in text: String,
    excluding taken: [ChatReferenceMatch]
) -> [ChatReferenceMatch] {
    guard let regex = taskKeyRegex else { return [] }
    let ns = text as NSString
    let full = NSRange(location: 0, length: ns.length)
    var results: [ChatReferenceMatch] = []
    for match in regex.matches(in: text, range: full) {
        if overlaps(match.range, taken, in: text) { continue }
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

// MARK: - Authored Project / evidence references

// The smallest authored contract for the two kinds with no natural syntax:
// `project:<slug>` (kebab-case PM slug) and `evidence:<token>` (an opaque
// commit / run / receipt / KR reference). The keyword is lowercase and must sit
// at a word boundary so it isn't caught mid-identifier.
private let projectPattern = "(?<![A-Za-z0-9])project:([a-z0-9][a-z0-9-]*)"
private let evidencePattern = "(?<![A-Za-z0-9])evidence:([A-Za-z0-9][A-Za-z0-9._/-]*)"

private let projectRegex = try? NSRegularExpression(pattern: projectPattern)
private let evidenceRegex = try? NSRegularExpression(pattern: evidencePattern)

private func authoredReferences(in text: String) -> [ChatReferenceMatch] {
    let ns = text as NSString
    let full = NSRange(location: 0, length: ns.length)
    var candidates: [ChatReferenceMatch] = []

    func collect(_ regex: NSRegularExpression?, kind: ChatReferenceKind) {
        guard let regex else { return }
        for match in regex.matches(in: text, range: full) {
            guard let wholeRange = Range(match.range, in: text) else { continue }
            candidates.append(ChatReferenceMatch(
                range: wholeRange,
                kind: kind,
                identifier: ns.substring(with: match.range(at: 1)),
                displayText: ns.substring(with: match.range)
            ))
        }
    }

    collect(projectRegex, kind: .project)
    collect(evidenceRegex, kind: .evidence)

    // The two syntaxes can share a span: `project:evidence:x` is a project
    // whose slug is `evidence` and an evidence whose token is `x`, overlapping
    // on `evidence`. Resolve by earliest start — the outer keyword claims the
    // range, so `project:evidence:x` is one project reference and
    // `evidence:project:foo` is one evidence reference — and on a tie the
    // longer span wins. A later candidate touching any kept span is dropped, so
    // authored references never overlap.
    candidates.sort {
        let a = NSRange($0.range, in: text)
        let b = NSRange($1.range, in: text)
        return a.location < b.location || (a.location == b.location && a.length > b.length)
    }
    var kept: [ChatReferenceMatch] = []
    var claimed: [NSRange] = []
    for candidate in candidates {
        let range = NSRange(candidate.range, in: text)
        if claimed.contains(where: { NSIntersectionRange($0, range).length > 0 }) { continue }
        kept.append(candidate)
        claimed.append(range)
    }
    return kept
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
