// ExitSignals — the launch journal's `exit` line for a catchable signal.
//
// A signal handler may only call async-signal-safe functions, so each line is
// formatted at launch and the handler stamps its time in place, then writes it
// with open(O_APPEND), write and close. The path is reopened, not a held fd,
// because trimming replaces the journal file. The handler then restores the
// previous disposition: a termination signal is raised again; a fault returns
// and re-faults into whoever handled it before (Ghostty's crash reporter, or
// the default crash report).

import Darwin
import Foundation
import Synchronization

enum ExitSignals {
    static let caught: [(number: Int32, name: String)] = [
        (SIGTERM, "SIGTERM"), (SIGHUP, "SIGHUP"), (SIGINT, "SIGINT"), (SIGQUIT, "SIGQUIT"), (SIGABRT, "SIGABRT"),
        (SIGSEGV, "SIGSEGV"), (SIGBUS, "SIGBUS"), (SIGILL, "SIGILL"), (SIGTRAP, "SIGTRAP"), (SIGFPE, "SIGFPE"),
    ]

    /// Where `at`'s value starts in every line: `{"at":"` is 7 bytes.
    static let timestampOffset = 7
    static let timestampPlaceholder = "0000-00-00T00:00:00Z"

    /// The NDJSON line written when `name` arrives, its time not yet stamped.
    static func line(_ fields: [String: String], signal name: String) -> [UInt8] {
        var fields = fields
        fields["signal"] = name
        // A dictionary of strings always serializes, and `signal` keeps it non-empty.
        let body = try! JSONSerialization.data(withJSONObject: fields, options: [.sortedKeys])
        return Array("{\"at\":\"\(timestampPlaceholder)\",".utf8) + body.dropFirst() + [0x0A]
    }

    /// Write `epoch` as `YYYY-MM-DDTHH:MM:SSZ` into 20 bytes; integer arithmetic only.
    static func stamp(_ epoch: Int, into out: UnsafeMutablePointer<UInt8>) {
        let days = epoch >= 0 ? epoch / 86_400 : (epoch - 86_399) / 86_400
        let seconds = epoch - days * 86_400
        // Civil date from days since 1970-01-01 (Howard Hinnant's algorithm).
        let z = days + 719_468
        let era = (z >= 0 ? z : z - 146_096) / 146_097
        let dayOfEra = z - era * 146_097
        let yearOfEra = (dayOfEra - dayOfEra / 1460 + dayOfEra / 36524 - dayOfEra / 146_096) / 365
        let dayOfYear = dayOfEra - (365 * yearOfEra + yearOfEra / 4 - yearOfEra / 100)
        let monthIndex = (5 * dayOfYear + 2) / 153
        let day = dayOfYear - (153 * monthIndex + 2) / 5 + 1
        let month = monthIndex < 10 ? monthIndex + 3 : monthIndex - 9
        let year = yearOfEra + era * 400 + (month <= 2 ? 1 : 0)
        func digits(_ value: Int, _ count: Int, at offset: Int) {
            var value = value
            for index in stride(from: offset + count - 1, through: offset, by: -1) {
                out[index] = UInt8(48 + value % 10)
                value /= 10
            }
        }
        digits(year, 4, at: 0)
        digits(month, 2, at: 5)
        digits(day, 2, at: 8)
        digits(seconds / 3600, 2, at: 11)
        digits(seconds / 60 % 60, 2, at: 14)
        digits(seconds % 60, 2, at: 17)
    }

    /// Install handlers that write `fields` plus the signal's name to `path`,
    /// once across every exit path sharing `flag`. A signal the process ignores stays ignored.
    static func install(path: String, flag: UnsafeMutablePointer<Atomic<Bool>>, fields: [String: String]) {
        let count = Int(NSIG)
        let state = ExitSignalState(
            path: strdup(path), flag: flag,
            lines: .allocate(capacity: count), lengths: .allocate(capacity: count),
            previous: .allocate(capacity: count))
        state.lines.initialize(repeating: nil, count: count)
        state.lengths.initialize(repeating: 0, count: count)
        state.previous.initialize(repeating: sigaction(), count: count)
        for (number, name) in caught {
            let line = line(fields, signal: name)
            let buffer = UnsafeMutablePointer<UInt8>.allocate(capacity: line.count)
            buffer.initialize(from: line, count: line.count)
            state.lines[Int(number)] = buffer
            state.lengths[Int(number)] = line.count
        }
        exitSignalState = state
        for (number, _) in caught {
            var action = sigaction()
            action.__sigaction_u.__sa_handler = handleExitSignal
            sigemptyset(&action.sa_mask)
            guard sigaction(number, &action, state.previous + Int(number)) == 0 else { continue }
            let previous = state.previous[Int(number)].__sigaction_u.__sa_handler
            if unsafeBitCast(previous, to: Int.self) == 1 {  // SIG_IGN
                sigaction(number, state.previous + Int(number), nil)
            }
        }
    }
}

/// Everything the handler touches, allocated before it can run; never freed.
private struct ExitSignalState {
    let path: UnsafeMutablePointer<CChar>?
    let flag: UnsafeMutablePointer<Atomic<Bool>>
    let lines: UnsafeMutablePointer<UnsafeMutablePointer<UInt8>?>
    let lengths: UnsafeMutablePointer<Int>
    let previous: UnsafeMutablePointer<sigaction>
}

nonisolated(unsafe) private var exitSignalState: ExitSignalState?

private func handleExitSignal(_ signal: Int32) {
    guard let state = exitSignalState else { return }
    let index = Int(signal)
    if state.flag.pointee.compareExchange(expected: false, desired: true, ordering: .sequentiallyConsistent).exchanged,
       let path = state.path, let line = state.lines[index] {
        // `ExitSignals.timestampOffset`, as a literal: the handler reads no lazily initialized global.
        ExitSignals.stamp(time(nil), into: line + 7)
        let fd = open(path, O_WRONLY | O_APPEND | O_CREAT, 0o644)
        if fd >= 0 {
            _ = write(fd, line, state.lengths[index])
            close(fd)
        }
    }
    sigaction(signal, state.previous + index, nil)
    switch signal {
    case SIGSEGV, SIGBUS, SIGILL, SIGTRAP, SIGFPE: return
    default: raise(signal)
    }
}
