import Darwin
import Foundation
import OSLog

/// What both apps' waits are measured with: one signposter (`studio.loopflow`,
/// category `perf`) that `log show --signpost` and Instruments read, and the
/// process's own start for the part of a cold start no interval can contain.
public enum PerfClock {
    public static let signposter = OSSignposter(subsystem: "studio.loopflow", category: "perf")

    /// Milliseconds from the kernel starting this process to now.
    public static func millisecondsSinceProcessStart() -> Double? {
        var info = kinfo_proc()
        var size = MemoryLayout<kinfo_proc>.stride
        var name = [CTL_KERN, KERN_PROC, KERN_PROC_PID, getpid()]
        guard sysctl(&name, UInt32(name.count), &info, &size, nil, 0) == 0 else { return nil }
        let start = info.kp_proc.p_starttime
        let started = Double(start.tv_sec) + Double(start.tv_usec) / 1_000_000
        return (Date().timeIntervalSince1970 - started) * 1000
    }
}

extension ContinuousClock.Instant {
    public var elapsedMs: Double {
        let elapsed = ContinuousClock.now - self
        return Double(elapsed.components.seconds) * 1000 + Double(elapsed.components.attoseconds) / 1e15
    }
}
