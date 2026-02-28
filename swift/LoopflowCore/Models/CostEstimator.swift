// Estimate token cost from known per-model rates or a global fallback.

import Foundation

public struct TokenCostRates: Sendable {
    public let inputPerMtok: Double
    public let outputPerMtok: Double
    public let cacheReadPerMtok: Double
    public let cacheWritePerMtok: Double
}

public struct CostEstimate: Sendable {
    /// Cost of input + output + reasoning tokens.
    public let nonCached: Double
    /// Cost of cache_read + cache_write tokens.
    public let cached: Double
    /// True when using global fallback rates (no model-specific match).
    public let isEstimate: Bool

    public var total: Double { nonCached + cached }
}

public enum CostEstimator {
    private static let modelRates: [(prefix: String, rates: TokenCostRates)] = [
        // Claude (published API rates — useful for equivalent-cost estimation)
        ("claude-opus-4", TokenCostRates(
            inputPerMtok: 15.0, outputPerMtok: 75.0,
            cacheReadPerMtok: 1.875, cacheWritePerMtok: 18.75)),
        ("claude-sonnet-4", TokenCostRates(
            inputPerMtok: 3.0, outputPerMtok: 15.0,
            cacheReadPerMtok: 0.30, cacheWritePerMtok: 3.75)),
        ("claude-haiku", TokenCostRates(
            inputPerMtok: 0.80, outputPerMtok: 4.0,
            cacheReadPerMtok: 0.08, cacheWritePerMtok: 1.0)),
        // OpenCode (mirrors rust/loopflow/src/lfd/providers.rs)
        ("kimi-k2", TokenCostRates(
            inputPerMtok: 0.60, outputPerMtok: 2.50,
            cacheReadPerMtok: 0.15, cacheWritePerMtok: 0.60)),
        ("qwen3-coder", TokenCostRates(
            inputPerMtok: 0.22, outputPerMtok: 1.0,
            cacheReadPerMtok: 0.22, cacheWritePerMtok: 0.22)),
        ("qwen3-max", TokenCostRates(
            inputPerMtok: 1.20, outputPerMtok: 6.0,
            cacheReadPerMtok: 1.20, cacheWritePerMtok: 1.20)),
    ]

    /// Global fallback — Sonnet-level as a middle ground.
    public static let globalDefault = TokenCostRates(
        inputPerMtok: 3.0, outputPerMtok: 15.0,
        cacheReadPerMtok: 0.30, cacheWritePerMtok: 3.75
    )

    public static func rates(for model: String?) -> TokenCostRates {
        guard let model else { return globalDefault }
        return modelRates.first { model.hasPrefix($0.prefix) }?.rates ?? globalDefault
    }

    public static func hasSpecificRates(for model: String) -> Bool {
        modelRates.contains { model.hasPrefix($0.prefix) }
    }

    /// Estimate cost from token totals.
    /// Reasoning tokens are priced at the output rate (standard for Claude/OpenAI).
    public static func estimateCost(_ tokens: TokenTotals, model: String? = nil) -> CostEstimate {
        let r = rates(for: model)
        let mtok = { (n: Int) in Double(n) / 1_000_000.0 }

        let nonCachedCost = mtok(tokens.input) * r.inputPerMtok
            + mtok(tokens.output) * r.outputPerMtok
            + mtok(tokens.reasoning) * r.outputPerMtok

        let cachedCost = mtok(tokens.cacheRead) * r.cacheReadPerMtok
            + mtok(tokens.cacheWrite) * r.cacheWritePerMtok

        let isEstimate = model.map { !hasSpecificRates(for: $0) } ?? true

        return CostEstimate(
            nonCached: nonCachedCost,
            cached: cachedCost,
            isEstimate: isEstimate
        )
    }
}
