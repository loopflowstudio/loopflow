// DesktopVitals — what the app was doing before it went away, in lf.log.
//
// Every 5 minutes one `vitals` line: memory footprint and its peak, open file
// descriptors, threads, and the `lf` reads since the last line (failed, and
// SIGKILLed on cancellation) with the busiest verbs. A storm of cancelled reads
// turns the line into a warning. Memory pressure and main-thread hangs log as
// they happen; a hang still running at 10 s logs before it ends, because
// force-quits follow hangs. The last line also bounds `lf doctor`'s system-log
// search for a launch that ended without a record.

import Darwin
import Foundation

final class DesktopVitals: @unchecked Sendable {
    static let interval: TimeInterval = 300
    /// More cancelled reads than this in one interval is a feedback loop, not use.
    static let stormKills = 100
    static let hangReported: TimeInterval = 2
    static let hangUnresponsive: TimeInterval = 10

    struct Sample: Equatable {
        var footprintMB: Int
        var peakMB: Int
        var fds: Int
        var threads: Int
    }

    private let log: DesktopLog
    private let journal: LaunchJournal
    private let queue = DispatchQueue(label: "studio.loopflow.desktop-vitals", qos: .utility)
    private var timer: DispatchSourceTimer?
    private var pressure: DispatchSourceMemoryPressure?

    init(log: DesktopLog = .home, journal: LaunchJournal = .home) {
        self.log = log
        self.journal = journal
    }

    static let home = DesktopVitals()

    /// Start the heartbeat, the memory-pressure source and the hang watchdog. Call once.
    func start() {
        let timer = DispatchSource.makeTimerSource(queue: queue)
        timer.schedule(deadline: .now() + Self.interval, repeating: Self.interval, leeway: .seconds(10))
        timer.setEventHandler { [self] in Self.beat(Self.sample(), journal.takeTally(), log: log) }
        timer.resume()
        self.timer = timer

        let pressure = DispatchSource.makeMemoryPressureSource(eventMask: [.warning, .critical], queue: queue)
        pressure.setEventHandler { [self, weak pressure] in
            guard let event = pressure?.data else { return }
            log.warn("memory pressure", ["level": event.contains(.critical) ? "critical" : "warning",
                                         "footprint_mb": String(Self.sample().footprintMB)])
        }
        pressure.resume()
        self.pressure = pressure

        let watchdog = Thread { [log] in Self.watchMainThread(log: log) }
        watchdog.name = "studio.loopflow.main-thread-watchdog"
        watchdog.qualityOfService = .utility
        watchdog.start()
    }

    /// Write one heartbeat for `sample` and the reads in `tally`.
    static func beat(_ sample: Sample, _ tally: LaunchJournal.Tally, log: DesktopLog) {
        let killed: Int = tally.killed.values.reduce(0, +)
        let reads: Int = tally.reads.values.reduce(0, +)
        let busiest = tally.reads.sorted { $0.value != $1.value ? $0.value > $1.value : $0.key < $1.key }
        let top: String = busiest.prefix(3).map { "\($0.key)×\($0.value)" }.joined(separator: ",")
        let storm = killed > stormKills
        let fields: KeyValuePairs<String, String> = [
            "footprint_mb": String(sample.footprintMB), "peak_mb": String(sample.peakMB),
            "fds": String(sample.fds), "threads": String(sample.threads),
            "reads": String(reads), "failed": String(tally.failed), "killed": String(killed), "top": top,
        ]
        log.write(storm ? .warn : .info, storm ? "lf reads cancelled at storm rate" : "vitals", fields)
    }

    static func sample() -> Sample {
        var vm = task_vm_info_data_t()
        var count = mach_msg_type_number_t(MemoryLayout<task_vm_info_data_t>.size / MemoryLayout<natural_t>.size)
        let status = withUnsafeMutablePointer(to: &vm) {
            $0.withMemoryRebound(to: integer_t.self, capacity: Int(count)) {
                task_info(mach_task_self_, task_flavor_t(TASK_VM_INFO), $0, &count)
            }
        }
        let megabyte: UInt64 = 1 << 20
        var task = proc_taskinfo()
        let taskBytes = proc_pidinfo(getpid(), PROC_PIDTASKINFO, 0, &task, Int32(MemoryLayout<proc_taskinfo>.size))
        return Sample(
            footprintMB: status == KERN_SUCCESS ? Int(vm.phys_footprint / megabyte) : -1,
            peakMB: status == KERN_SUCCESS ? Int(UInt64(max(vm.ledger_phys_footprint_peak, 0)) / megabyte) : -1,
            fds: openFileDescriptors(),
            threads: taskBytes > 0 ? Int(task.pti_threadnum) : -1)
    }

    private static func openFileDescriptors() -> Int {
        let size = proc_pidinfo(getpid(), PROC_PIDLISTFDS, 0, nil, 0)
        guard size > 0 else { return -1 }
        let capacity = Int(size) / MemoryLayout<proc_fdinfo>.stride
        var fds = [proc_fdinfo](repeating: proc_fdinfo(), count: capacity)
        let used = fds.withUnsafeMutableBytes { proc_pidinfo(getpid(), PROC_PIDLISTFDS, 0, $0.baseAddress, size) }
        return used > 0 ? Int(used) / MemoryLayout<proc_fdinfo>.stride : -1
    }

    /// Ping the main queue each second; log hangs of 2 s or more when they end,
    /// and one still running at 10 s at once.
    private static func watchMainThread(log: DesktopLog) {
        while true {
            let answered = DispatchSemaphore(value: 0)
            let sent = ProcessInfo.processInfo.systemUptime
            DispatchQueue.main.async { answered.signal() }
            if answered.wait(timeout: .now() + hangReported) == .timedOut {
                if answered.wait(timeout: .now() + (hangUnresponsive - hangReported)) == .timedOut {
                    log.warn("main thread unresponsive", ["seconds": String(Int(hangUnresponsive))])
                    answered.wait()
                }
                let hung = ProcessInfo.processInfo.systemUptime - sent
                log.warn("main thread hung", ["ms": String(Int(hung * 1000))])
            }
            Thread.sleep(forTimeInterval: 1)
        }
    }
}
