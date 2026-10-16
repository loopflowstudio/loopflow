// The operational lens: a small recessed glass light. A controlled inner glow
// when lit, a faint specular highlight, and reflective near-black glass when
// off. The HAL allusion stays implicit — no eye housing, no pulsing, no novelty.
// Rendering only; the color and reason are decided by `WaveLens` (W2-123 owns
// the final projection).

import SwiftUI
import Loopflow

struct WaveLensView: View {
    let lens: WaveLens
    var diameter: CGFloat = 11
    var accessibilityId: String = "wave-lens"

    @Environment(\.palette) private var palette

    var body: some View {
        let color = lens.color.ink(in: palette)
        let lit = lens.color.isLit
        Circle()
            .fill(
                RadialGradient(
                    colors: lit
                        ? [color, color.opacity(0.55), palette.background]
                        : [palette.surfaceElement, palette.background],
                    center: UnitPoint(x: 0.38, y: 0.34),
                    startRadius: 0,
                    endRadius: diameter * 0.75
                )
            )
            .overlay(
                // Faint specular highlight — glass, not a flat dot.
                Circle()
                    .fill(palette.text.opacity(lit ? 0.5 : 0.18))
                    .frame(width: diameter * 0.26, height: diameter * 0.26)
                    .blur(radius: 0.5)
                    .offset(x: -diameter * 0.2, y: -diameter * 0.22)
            )
            .overlay(
                // Recessed rim.
                Circle().strokeBorder(palette.borderStrong, lineWidth: 0.75)
            )
            .frame(width: diameter, height: diameter)
            .shadow(color: lit ? color.opacity(0.6) : .clear, radius: lit ? 2.5 : 0)
            .accessibilityElement()
            .accessibilityLabel("Status: \(lens.reason)")
            .accessibilityValue("\(lens.color.rawValue) lens")
            .accessibilityIdentifier(accessibilityId)
            .help(lens.reason)
    }
}
