#if os(macOS)
import SwiftUI
import Testing
import ViewInspector
@testable import Loopflow
@testable import LoopflowMac

@Suite("Desktop semantic colors")
struct WorkPaletteTests {
    @Test("Text and indicators retain contrast on actual composited surfaces", arguments: [false, true])
    func contrast(dark: Bool) {
        let palette: LoopflowPalette = dark ? .dark : .light
        var environment = EnvironmentValues()
        environment.colorScheme = dark ? .dark : .light
        let surfaces = [palette.background, palette.surface, palette.surfaceMuted, palette.surfaceElement]
        let text = [palette.text, palette.textSecondary, palette.textTertiary, palette.linkInk,
                    palette.linkHover, palette.actionInk, palette.emphasisInk, palette.working,
                    palette.waiting, palette.failure, palette.success]
        for surface in surfaces {
            let base = rgb(surface, environment)
            let selected = composite(rgb(palette.selectionFill, environment), over: base)
            for background in [base, selected] {
                for ink in text {
                    #expect(ratio(rgb(ink, environment), background) >= 4.5,
                            "\(dark ? "dark" : "light") text \(ink) on \(background)")
                }
                for ink in [palette.selectionEdge, palette.keyboardFocus] {
                    #expect(ratio(rgb(ink, environment), background) >= 3)
                }
            }
        }
    }

    @Test("Selection, focus, links and failures have independent roles", arguments: [false, true])
    func independentRoles(dark: Bool) {
        let palette: LoopflowPalette = dark ? .dark : .light
        #expect(palette.linkInk == palette.textSecondary)
        #expect(palette.linkHover == palette.text)
        #expect(palette.actionInk != palette.emphasisInk)
        #expect(palette.selectionEdge != palette.failure)
        #expect(palette.keyboardFocus != palette.selectionEdge)
        #expect(palette.keyboardFocus != palette.failure)
        #expect(WorkTone.blocked.ink(in: palette) == palette.failure)
        #expect(WorkTone.human.ink(in: palette) == palette.waiting)
        #expect(WorkTone.running.ink(in: palette) == palette.working)
    }

    @Test("Keyboard focus stays visible around a selected failed row", arguments: [false, true])
    @MainActor
    func focusWithSelectedFailure(focused: Bool) throws {
        let view = Text("Error")
            .foregroundStyle(LoopflowPalette.light.failure)
            .workSelection(true)
            .keyboardFocusRing(focused)
        let rings = try view.inspect().findAll(ViewType.Shape.self).filter {
            (try? $0.opacity()) != nil
        }
        let ring = try #require(rings.first)
        #expect(try ring.opacity() == (focused ? 1 : 0))
        #expect(try ring.strokeStyle().lineWidth == 2)
        #expect(try ring.allowsHitTesting() == false)
        #expect(try view.inspect().find(text: "Error").string() == "Error")
    }

    private func rgb(_ color: Color, _ environment: EnvironmentValues) -> [Double] {
        let resolved = color.resolve(in: environment)
        return [Double(resolved.red), Double(resolved.green), Double(resolved.blue), Double(resolved.opacity)]
    }

    private func composite(_ foreground: [Double], over background: [Double]) -> [Double] {
        (0..<3).map { foreground[$0] * foreground[3] + background[$0] * (1 - foreground[3]) } + [1]
    }

    private func ratio(_ a: [Double], _ b: [Double]) -> Double {
        func luminance(_ color: [Double]) -> Double {
            let linear = color.prefix(3).map { $0 <= 0.04045 ? $0 / 12.92 : pow(($0 + 0.055) / 1.055, 2.4) }
            return linear[0] * 0.2126 + linear[1] * 0.7152 + linear[2] * 0.0722
        }
        let first = luminance(a), second = luminance(b)
        return (max(first, second) + 0.05) / (min(first, second) + 0.05)
    }
}
#endif
