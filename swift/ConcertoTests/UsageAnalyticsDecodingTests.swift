import Foundation
import Testing
@testable import LoopflowCore

@Suite("Usage analytics decoding")
struct UsageAnalyticsDecodingTests {
    @Test("timeseries payload decodes snake_case fields")
    func decodeTimeseries() throws {
        let data = Data(
            """
            {
              "object": "usage_timeseries",
              "bucket": "day",
              "group_by": "wave",
              "from": "2026-02-01T00:00:00Z",
              "to": "2026-02-28T00:00:00Z",
              "buckets": [
                {
                  "period": "2026-02-01",
                  "groups": [
                    {
                      "key": "engbot",
                      "tokens": {
                        "input": 45200,
                        "output": 8100,
                        "reasoning": 1200,
                        "cache_read": 900,
                        "cache_write": 300
                      },
                      "sessions": 3,
                      "turns": 12
                    }
                  ]
                }
              ]
            }
            """.utf8
        )

        let decoded = try JSONDecoder().decode(UsageTimeseries.self, from: data)
        #expect(decoded.object == "usage_timeseries")
        #expect(decoded.bucket == "day")
        #expect(decoded.groupBy == "wave")
        #expect(decoded.buckets.count == 1)
        #expect(decoded.buckets[0].groups[0].tokens.cacheRead == 900)
    }

    @Test("summary payload decodes snake_case fields")
    func decodeSummary() throws {
        let data = Data(
            """
            {
              "object": "usage_summary",
              "group_by": "source",
              "groups": [
                {
                  "key": "diff",
                  "tokens": {
                    "input": 1000,
                    "output": 0,
                    "reasoning": 0,
                    "cache_read": 0,
                    "cache_write": 0
                  },
                  "sessions": 2,
                  "turns": 0
                }
              ]
            }
            """.utf8
        )

        let decoded = try JSONDecoder().decode(UsageSummary.self, from: data)
        #expect(decoded.groupBy == "source")
        #expect(decoded.groups[0].key == "diff")
        #expect(decoded.groups[0].tokens.total == 1000)
    }
}
