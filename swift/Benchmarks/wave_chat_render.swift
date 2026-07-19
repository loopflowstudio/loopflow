// Baseline for the Wave Chat token path's render stage: what one provider text
// delta costs the Mac surface's main actor.
//
//   swift <this-file> scratch/turn626.json
//
// The listener broadcasts the whole open turn on every delta (wave/runtime.rs
// `append_turn_item_locked`), so per token the Mac does two things on the main
// thread, both sized by the turn rather than by the delta:
//
//   parse   the entire turn frame — prose plus every tool output already
//           parsed on the previous token (WaveChatClient.handle)
//   layout  the entire turn's text — NSTextView.string is replaced whole and
//           intrinsicContentSize forces ensureLayout
//           (SelectableAssistantMessageTextView.updateNSView)
//
// Both are replayed here against a real recorded turn, delta by delta, so the
// reported rate is the rate a reader would have seen.

import AppKit
import Foundation

struct RecordedTurn: Decodable {
    let turn: String
    let deltas: [String]
    let items: [AnyItem]

    struct AnyItem: Decodable, Encodable {
        let raw: [String: JSONValue]
        init(from decoder: Decoder) throws {
            raw = try decoder.singleValueContainer().decode([String: JSONValue].self)
        }
        func encode(to encoder: Encoder) throws {
            var c = encoder.singleValueContainer()
            try c.encode(raw)
        }
    }
}

enum JSONValue: Codable {
    case string(String), number(Double), bool(Bool), null
    case array([JSONValue]), object([String: JSONValue])

    init(from decoder: Decoder) throws {
        let c = try decoder.singleValueContainer()
        if c.decodeNil() { self = .null }
        else if let v = try? c.decode(Bool.self) { self = .bool(v) }
        else if let v = try? c.decode(Double.self) { self = .number(v) }
        else if let v = try? c.decode(String.self) { self = .string(v) }
        else if let v = try? c.decode([JSONValue].self) { self = .array(v) }
        else { self = .object(try c.decode([String: JSONValue].self)) }
    }

    func encode(to encoder: Encoder) throws {
        var c = encoder.singleValueContainer()
        switch self {
        case .null: try c.encodeNil()
        case .bool(let v): try c.encode(v)
        case .number(let v): try c.encode(v)
        case .string(let v): try c.encode(v)
        case .array(let v): try c.encode(v)
        case .object(let v): try c.encode(v)
        }
    }
}

let path = CommandLine.arguments.count > 1 ? CommandLine.arguments[1] : "scratch/turn626.json"
let recorded = try JSONDecoder().decode(
    RecordedTurn.self, from: Data(contentsOf: URL(fileURLWithPath: path)))

// The turn view the surface actually renders: prose plus the accumulated items.
let textView = NSTextView(frame: NSRect(x: 0, y: 0, width: 720, height: 100))
textView.isEditable = false
textView.font = .systemFont(ofSize: 13)

var accumulated = ""
var parseUs: [Double] = []
var layoutUs: [Double] = []
var frameBytes = 0

for delta in recorded.deltas {
    accumulated += delta

    // The frame the listener broadcasts for this delta.
    let frame = try JSONEncoder().encode(
        TurnFrame(id: recorded.turn, text: accumulated, items: recorded.items))
    frameBytes += frame.count

    var start = DispatchTime.now()
    _ = try JSONSerialization.jsonObject(with: frame)
    parseUs.append(elapsedUs(start))

    // The render the frame triggers: whole string replaced, layout forced.
    start = DispatchTime.now()
    textView.string = accumulated
    textView.layoutManager?.ensureLayout(for: textView.textContainer!)
    _ = textView.layoutManager?.usedRect(for: textView.textContainer!)
    layoutUs.append(elapsedUs(start))
}

struct TurnFrame: Encodable {
    let id: String
    let text: String
    let items: [RecordedTurn.AnyItem]
}

func elapsedUs(_ start: DispatchTime) -> Double {
    Double(DispatchTime.now().uptimeNanoseconds - start.uptimeNanoseconds) / 1e3
}

func report(_ label: String, _ samples: [Double]) {
    let sorted = samples.sorted()
    func pct(_ p: Double) -> Double { sorted[min(Int(Double(sorted.count) * p), sorted.count - 1)] }
    print(
        String(
            format: "  %-8@ median %8.1f us   p90 %8.1f us   max %8.1f us", label as NSString,
            pct(0.5), pct(0.9), pct(1.0)))
}

let total = zip(parseUs, layoutUs).map(+)
let totalMs = total.reduce(0, +) / 1e3

print("== recorded turn \(recorded.turn)")
print("  deltas           \(recorded.deltas.count)")
print("  prose chars      \(accumulated.count)")
print("  non-text items   \(recorded.items.count)")
print("  frame bytes      \(frameBytes) (\(frameBytes / max(accumulated.count, 1))x the prose)")

print("\n== main-actor cost per delta")
report("parse", parseUs)
report("layout", layoutUs)
report("total", total)

let firstTenth = Array(total.prefix(total.count / 10))
let lastTenth = Array(total.suffix(total.count / 10))
print("\n== decay across the turn")
report("first10%", firstTenth)
report("last10%", lastTenth)

print("\n== visible rate")
print(String(format: "  render time      %.0f ms of main-actor work for one turn", totalMs))
print(
    String(
        format: "  sustained        %.1f deltas/s (%.1f words/s at %.1f chars/delta)",
        Double(total.count) / (totalMs / 1e3),
        Double(total.count) / (totalMs / 1e3) / 1.4,
        Double(accumulated.count) / Double(total.count)))
