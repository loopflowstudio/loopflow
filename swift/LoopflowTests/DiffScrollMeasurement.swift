#if os(macOS)
import AppKit
import Foundation
import SwiftUI
import Testing
@testable import Loopflow
@testable import LoopflowMac

/// Opt-in: how long the Diff takes to draw each scrolled frame and each
/// sidebar jump over a real Task's comparison, read through a built `lf`.
///
///     LF_DIFF_SCROLL_LF=$PWD/target/debug/lf LF_DIFF_SCROLL_ISSUE=LOO-515 \
///       swift test --package-path swift --filter DiffScrollMeasurement
@Suite("Diff scrolling measurement", .requiresDisplay, .serialized)
@MainActor
struct DiffScrollMeasurement {
    @Test(.enabled(if: ProcessInfo.processInfo.environment["LF_DIFF_SCROLL_LF"] != nil))
    func scrollAndJumpThroughATasksDiff() async throws {
        let environment = ProcessInfo.processInfo.environment
        let lf = try #require(environment["LF_DIFF_SCROLL_LF"])
        let issue = try #require(environment["LF_DIFF_SCROLL_ISSUE"])
        let query = RegistryQuery { args, cwd in try Self.run(lf, args, cwd: cwd) }
        let store = TaskFilesStore(issue: issue, cwd: FileManager.default.currentDirectoryPath, query: query)
        store.autosave = false
        // Every file shown, the way a long review reads.
        store.diffReview.hidden = []
        await store.diffReview.loadReview()
        let review = try #require(store.diffReview.shownReview)
        let lines = review.files.reduce(0) { $0 + $1.blocks.count }
        let parsed = store.diffReview.stops(.changes).count, counted = review.files.reduce(0) { $0 + store.diffReview.changeCount($1) }
        for file in review.files {
            let stops = store.diffReview.stops(of: [file], in: .changes).count
            guard stops != store.diffReview.changeCount(file) else { continue }
            print("count differs: \(file.path) parsed \(stops) counted \(store.diffReview.changeCount(file))")
        }

        let window = NSWindow(contentRect: CGRect(x: 0, y: 0, width: 600, height: 900),
                              styleMask: [.titled, .resizable], backing: .buffered, defer: false)
        window.isReleasedWhenClosed = false
        window.contentView = NSHostingView(rootView: TaskChangesView(store: store, task: nil, isFocused: true,
                                                                     addToConversation: nil))
        window.orderFront(nil)
        defer { window.contentView = nil; window.close() }
        let first = window.measureFrame {}
        let scroll = try #require(window.contentView?.tallestScrollView)

        var scrolled: [Double] = []
        for _ in 0..<300 {
            let y = scroll.contentView.bounds.minY + 40
            scrolled.append(window.measureFrame {
                scroll.contentView.scroll(to: CGPoint(x: 0, y: y))
                scroll.reflectScrolledClipView(scroll.contentView)
            })
        }
        var jumps: [Double] = []
        var slowest: [(Double, String)] = []
        let targets = stride(from: 0, to: review.files.count, by: max(1, review.files.count / 20)).map { review.files[$0] }
        for file in targets.reversed() {
            jumps.append(window.measureFrame {
                store.diffReview[place: .changes] = .init(anchor: store.diffReview.topID(of: file, in: .changes), current: store.diffReview.stops(of: [file], in: .changes).first?.id)
            })
            slowest.append((jumps.last ?? 0, "\(file.path) (\(file.blocks.count) blocks)"))
        }
        let last = try #require(review.files.last)
        _ = window.measureFrame { store.diffReview[place: .changes] = .init(anchor: store.diffReview.topID(of: last, in: .changes), current: store.diffReview.stops(of: [last], in: .changes).first?.id) }
        let before = store.diffReview[place: .changes].anchor
        let earlier = window.measureFrame { store.diffReview.showEarlierFiles() }
        let held = store.diffReview[place: .changes].anchor == before
        let report = """
        files \(review.files.count), blocks \(lines), stops parsed \(parsed) counted \(counted), first frame \(Self.ms(first)) ms
        scroll frames (40 pt): median \(Self.ms(Self.percentile(scrolled, 0.5))) p95 \(Self.ms(Self.percentile(scrolled, 0.95))) max \(Self.ms(scrolled.max() ?? 0)) ms, over 16.7 ms: \(scrolled.count { $0 > 16.7 })/\(scrolled.count)
        sidebar jumps: median \(Self.ms(Self.percentile(jumps, 0.5))) p95 \(Self.ms(Self.percentile(jumps, 0.95))) max \(Self.ms(jumps.max() ?? 0)) ms
        files above: \(Self.ms(earlier)) ms, top line held: \(held), window now starts at file \(store.diffReview.window.lowerBound)
        slowest jumps: \(slowest.sorted { $0.0 > $1.0 }.prefix(3).map { "\(Self.ms($0.0)) ms to \($0.1)" }.joined(separator: ", "))
        """
        print(report)
        if let output = environment["LF_DIFF_SCROLL_OUTPUT"] {
            try report.write(toFile: output, atomically: true, encoding: .utf8)
        }
    }

    private nonisolated static func run(_ lf: String, _ args: [String], cwd: String?) throws -> String {
        let process = Process()
        process.executableURL = URL(fileURLWithPath: lf)
        process.arguments = args
        if let cwd { process.currentDirectoryURL = URL(fileURLWithPath: cwd) }
        let output = Pipe()
        process.standardOutput = output
        process.standardError = FileHandle.nullDevice
        try process.run()
        let data = output.fileHandleForReading.readDataToEndOfFile()
        process.waitUntilExit()
        guard process.terminationStatus == 0 else { throw RegistryQueryError("lf \(args.joined(separator: " ")) failed") }
        return String(decoding: data, as: UTF8.self)
    }

    private static func percentile(_ values: [Double], _ p: Double) -> Double {
        let sorted = values.sorted()
        return sorted.isEmpty ? 0 : sorted[min(sorted.count - 1, Int(Double(sorted.count) * p))]
    }

    private static func ms(_ value: Double) -> String { String(format: "%.1f", value) }
}
#endif
