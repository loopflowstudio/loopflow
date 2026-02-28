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

@Suite("Cost estimator")
struct CostEstimatorTests {
    @Test("known model uses specific rates")
    func knownModel() {
        let tokens = TokenTotals.fixture(input: 1_000_000, output: 500_000)
        let cost = CostEstimator.estimateCost(tokens, model: "claude-sonnet-4-20250514")

        // input: 1M × $3/Mtok = $3.00, output: 0.5M × $15/Mtok = $7.50
        #expect(cost.nonCached == 10.50)
        #expect(cost.cached == 0.0)
        #expect(!cost.isEstimate)
    }

    @Test("unknown model falls back to global default")
    func unknownModel() {
        let tokens = TokenTotals.fixture(input: 1_000_000)
        let cost = CostEstimator.estimateCost(tokens, model: "mystery-model-v9")

        // Falls back to Sonnet rates: 1M × $3/Mtok = $3.00
        #expect(cost.nonCached == 3.0)
        #expect(cost.isEstimate)
    }

    @Test("nil model is estimated")
    func nilModel() {
        let cost = CostEstimator.estimateCost(TokenTotals.fixture(), model: nil)
        #expect(cost.isEstimate)
    }

    @Test("two-tier grouping: cached vs non-cached")
    func twoTiers() {
        let tokens = TokenTotals.fixture(
            input: 2_000_000, output: 1_000_000, reasoning: 500_000,
            cacheRead: 3_000_000, cacheWrite: 100_000
        )
        let cost = CostEstimator.estimateCost(tokens, model: "claude-sonnet-4")

        // non-cached: 2M×$3 + 1M×$15 + 0.5M×$15 = $6 + $15 + $7.50 = $28.50
        // cached: 3M×$0.30 + 0.1M×$3.75 = $0.90 + $0.375 = $1.275
        #expect(abs(cost.nonCached - 28.50) < 0.001)
        #expect(abs(cost.cached - 1.275) < 0.001)
    }

    @Test("token totals two-tier properties")
    func tokenTotalsTwoTier() {
        let tokens = TokenTotals.fixture(
            input: 100, output: 50, reasoning: 10, cacheRead: 30, cacheWrite: 5
        )
        #expect(tokens.nonCached == 160)
        #expect(tokens.cached == 35)
        #expect(tokens.total == 195)
    }
}

extension TokenTotals {
    static func fixture(
        input: Int = 0, output: Int = 0, reasoning: Int = 0,
        cacheRead: Int = 0, cacheWrite: Int = 0
    ) -> TokenTotals {
        let data = Data("""
        {"input":\(input),"output":\(output),"reasoning":\(reasoning),"cache_read":\(cacheRead),"cache_write":\(cacheWrite)}
        """.utf8)
        return try! JSONDecoder().decode(TokenTotals.self, from: data)
    }
}
