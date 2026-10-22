#if os(macOS)
import Foundation

/// Edits use UTF-16 coordinates, matching NSTextView selection and Undo.
struct TaskTextEdit: Sendable {
    let range: NSRange
    let replacement: String

    static func between(_ before: String, _ after: String) -> [Self] {
        if before == after { return [] }
        func lines(_ text: String) -> [String] {
            let parts = text.components(separatedBy: "\n")
            return parts.enumerated().map { $0.element + ($0.offset < parts.count - 1 ? "\n" : "") }
        }
        let blocks = differences(lines(before), lines(after))
        let source = before as NSString
        return blocks.flatMap { block in
            let old = source.substring(with: block.range)
            // Trim equal edges before token matching: repeated letters must not
            // turn a prefix insertion into several artificial overlapping edits.
            let lhs = Array(old), rhs = Array(block.replacement)
            var prefix = 0, suffix = 0
            while prefix < min(lhs.count, rhs.count), lhs[prefix] == rhs[prefix] { prefix += 1 }
            while suffix < min(lhs.count, rhs.count) - prefix,
                  lhs[lhs.count - suffix - 1] == rhs[rhs.count - suffix - 1] { suffix += 1 }
            let offset = String(lhs.prefix(prefix)).utf16.count
            let before = String(lhs[prefix..<(lhs.count - suffix)])
            let after = String(rhs[prefix..<(rhs.count - suffix)])
            // Words make a replacement such as one → local one edit, rather
            // than preserving a stray 'l' when disk replaces that same word.
            return differences(tokens(before), tokens(after)).map {
                Self(range: NSRange(location: block.range.location + offset + $0.range.location, length: $0.range.length),
                     replacement: $0.replacement)
            }
        }
    }

    private static func tokens(_ text: String) -> [String] {
        var result: [String] = [], word = ""
        for character in text {
            if character.isLetter || character.isNumber || character == "_" {
                word.append(character)
            } else {
                if !word.isEmpty { result.append(word); word = "" }
                result.append(String(character))
            }
        }
        if !word.isEmpty { result.append(word) }
        return result
    }

    private static func differences(_ old: [String], _ new: [String]) -> [Self] {
        let difference = new.difference(from: old)
        let removed = Set(difference.removals.map { change in
            if case .remove(let offset, _, _) = change { return offset }
            preconditionFailure("Removal collection contains an insertion")
        })
        let inserted = Set(difference.insertions.map { change in
            if case .insert(let offset, _, _) = change { return offset }
            preconditionFailure("Insertion collection contains a removal")
        })
        var edits: [Self] = []
        var i = 0, j = 0, offset = 0
        while i < old.count || j < new.count {
            let start = offset
            var replacement = ""
            while i < old.count, removed.contains(i) { offset += old[i].utf16.count; i += 1 }
            while j < new.count, inserted.contains(j) { replacement += new[j]; j += 1 }
            if offset != start || !replacement.isEmpty {
                edits.append(Self(range: NSRange(location: start, length: offset - start), replacement: replacement))
            }
            if i < old.count, j < new.count, !removed.contains(i), !inserted.contains(j) {
                offset += old[i].utf16.count; i += 1; j += 1
            }
        }
        return edits
    }

    static func applying(_ edits: [Self], to text: String) -> String {
        let result = NSMutableString(string: text)
        for edit in edits.reversed() { result.replaceCharacters(in: edit.range, with: edit.replacement) }
        return result as String
    }

    static func merge(base: String, local: String, disk: String) -> String {
        if local == base || local == disk { return disk }
        if disk == base { return local }
        let localEdits = between(base, local), diskEdits = between(base, disk)
        let surviving = localEdits.filter { edit in
            !diskEdits.contains { incoming in
                let a = edit.range, b = incoming.range
                return max(a.location, b.location) < min(NSMaxRange(a), NSMaxRange(b))
                    || (a.length == 0 && b.length == 0 && a.location == b.location)
                    || (a.length == 0 && a.location > b.location && a.location < NSMaxRange(b))
                    || (b.length == 0 && b.location > a.location && b.location < NSMaxRange(a))
            }
        }
        return applying((surviving + diskEdits).sorted {
            ($0.range.location, $0.range.length) < ($1.range.location, $1.range.length)
        }, to: base)
    }
}
#endif
