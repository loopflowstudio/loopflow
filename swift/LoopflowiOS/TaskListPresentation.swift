import Foundation
import Loopflow

/// Presentation of existing Work facts; no client-owned Task state.
extension WorkViewPlan.Content {
    var mobileStatus: String {
        if primarySession == nil { return "No Session" }
        if !unavailable.isEmpty { return "Unavailable" }
        if states.contains(.blocked) { return "Needs a response" }
        if states.contains(.busy) { return "Working" }
        if states.contains(.available) { return "Ready" }
        if states.contains(.shipping) { return "Shipping" }
        return "Open"
    }

    var mobileStatusSymbol: String {
        if primarySession == nil || !unavailable.isEmpty { return "exclamationmark.circle" }
        if states.contains(.blocked) { return "bubble.left" }
        if states.contains(.busy) { return "circle.dotted" }
        return "circle"
    }

    func matchesMobileSearch(_ search: String) -> Bool {
        let term = search.trimmingCharacters(in: .whitespacesAndNewlines)
        return term.isEmpty || title.localizedStandardContains(term)
            || location?.cwd.localizedStandardContains(term) == true
    }
}
