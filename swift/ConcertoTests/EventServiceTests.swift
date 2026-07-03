import Foundation
import Testing
@testable import LoopflowCore

@Suite("Event service parsing")
struct EventServiceTests {
    @Test("wave_waiting parses session id")
    func parseWaitingEventWithSession() throws {
        let text = """
        {
          "type": "wave_waiting",
          "wave_id": "wave-1",
          "run_id": "run-1",
          "step": "design",
          "session_id": "terminal-1",
          "timestamp": "2026-03-17T12:00:00.000Z"
        }
        """

        let event = try parseWaveEvent(text)

        #expect(event.type == .waiting)
        #expect(event.waveId == "wave-1")
        #expect(event.runId == "run-1")
        #expect(event.step == "design")
        #expect(event.sessionId == "terminal-1")
    }
}

private enum EventParseError: Error {
    case expectedWaveEvent
}

private func parseWaveEvent(_ text: String) throws -> WaveEvent {
    guard let event = EventService.parseEvent(text: text),
          case .wave(let waveEvent) = event else {
        throw EventParseError.expectedWaveEvent
    }
    return waveEvent
}
