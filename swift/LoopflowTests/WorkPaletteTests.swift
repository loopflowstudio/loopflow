#if os(macOS)
import SwiftUI
import Testing
import ViewInspector
@testable import Loopflow
@testable import LoopflowMac

@Suite("Desktop semantic colors")
struct WorkPaletteTests {
    @Test("Text and indicators retain contrast on actual composited surfaces", arguments: ThemeDefinition.builtins.map(\.id) + ["171924/CAD2EF", "FFF4DF/34291F", "330909/FFEEDD", "747474/FFFFFF", "888888/000000"])
    func contrast(theme: String) throws {
        let definition: ThemeDefinition
        if let builtin = ThemeDefinition.builtins.first(where: { $0.id == theme }) { definition = builtin }
        else {
            let pair = theme.split(separator: "/")
            definition = try ThemeFile.parse(Data("background=\(pair[0])\nforeground=\(pair[1])".utf8), name: theme).theme
        }
        let dark = definition.isDark
        let palette = definition.palette
        var environment = EnvironmentValues()
        environment.colorScheme = dark ? .dark : .light
        let surfaces = [palette.background, palette.surface, palette.surfaceMuted, palette.surfaceElement]
        let text = [palette.text, palette.textSecondary, palette.textTertiary, palette.linkInk,
                    palette.linkHover, palette.actionInk, palette.emphasisInk, palette.accentInk, palette.working,
                    palette.waiting, palette.failure, palette.success]
        for tone in [WorkTone.running, .done, .waiting, .failure, .stopped] {
            #expect(ratio(rgb(tone.text(in: palette), environment), rgb(tone.fill(in: palette), environment)) >= 4.5)
        }
        let workSelection = rgb(palette.workSelectedFill, environment)
        for ink in [palette.workSelectedText, palette.workSelectedSecondaryText] {
            #expect(ratio(rgb(ink, environment), workSelection) >= 4.5)
        }
        #expect(definition.terminal.readabilityErrors.isEmpty)
        for surface in surfaces {
            let base = rgb(surface, environment)
            #expect(ratio(rgb(palette.selectionEdge, environment), base) >= 3)
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
        #expect(WorkTone.failure.ink(in: palette) == palette.failure)
        #expect(WorkTone.waiting.ink(in: palette) == palette.waiting)
        #expect(WorkTone.running.ink(in: palette) == palette.working)
    }

    @Test("Status chip colors follow the supplied palette, independently of native appearance")
    func chipPalette() {
        var environment = EnvironmentValues()
        environment.colorScheme = .light
        for tone in [WorkTone.running, .done, .waiting, .failure, .stopped] {
            let darkFill = rgb(tone.fill(in: .dark), environment)
            let lightFill = rgb(tone.fill(in: .light), environment)
            #expect(darkFill != lightFill)
            #expect(ratio(rgb(tone.text(in: .dark), environment), darkFill) >= 4.5)
            #expect(ratio(rgb(tone.text(in: .light), environment), lightFill) >= 4.5)
        }
    }

    @Test("Status wording, selection and focus survive every appearance", arguments: ThemeFixture.allCases)
    @MainActor
    func statusWithSelectionAndFocus(theme: ThemeFixture) throws {
        let appearance = AppearanceStore(paths: .temporary(), systemDark: false)
        try theme.preview(in: appearance)
        let palette = appearance.effectiveTheme.palette
        #expect(palette.selectionEdge != palette.failure)
        #expect(palette.keyboardFocus != palette.selectionEdge)
        #expect(palette.keyboardFocus != palette.failure)
        for state in [ProgramStatusState.working, .blocked, .error, .done, .idle] {
            let report = ProgramStatusReport(state: state, id: nil, kind: nil, progress: nil,
                                             app: nil, title: nil, msg: nil)
            for selected in [false, true] {
                for focused in [false, true] {
                    let view = ProgramStatusLabel(status: ProgramStatusRecords(seen: true, records: [report]))
                        .workSelection(selected)
                        .keyboardFocusRing(focused)
                        .modifier(AppAppearance(appearance: appearance))
                    let inspected = try view.inspect()
                    #expect(try inspected.environment(\.palette).keyboardFocus == palette.keyboardFocus)
                    #expect(try inspected.find(text: state.rawValue.capitalized).string() == state.rawValue.capitalized)
                    let shapes = inspected.findAll(ViewType.Shape.self)
                    let ring = try #require(shapes.first { (try? $0.opacity()) != nil })
                    #expect(try ring.opacity() == (focused ? 1 : 0))
                    #expect(try ring.strokeStyle().lineWidth == 2)
                    #expect(!ring.allowsHitTesting())
                    let edges = shapes.filter { (try? $0.fixedWidth()) == 2 }
                    #expect(edges.count == (selected ? 1 : 0))
                }
            }
        }
        let unknown = ProgramStatusLabel(status: ProgramStatusRecords(seen: false, records: []))
            .modifier(AppAppearance(appearance: appearance))
        #expect(try unknown.inspect().findAll(ViewType.Text.self).isEmpty)
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
