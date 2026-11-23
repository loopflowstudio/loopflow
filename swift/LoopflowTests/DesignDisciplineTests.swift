import Foundation
import Testing

@Suite("Design discipline")
struct DesignDisciplineTests {
    private static let swiftRoot = URL(fileURLWithPath: #filePath)
        .deletingLastPathComponent()
        .deletingLastPathComponent()

    private typealias Rule = (pattern: String, reason: String, outsideDesignOnly: Bool)

    /// Every line of Swift under `directories` that matches a rule, comments aside.
    private static func violations(in directories: [String], _ rules: [Rule]) throws -> [String] {
        var violations: [String] = []
        for directory in directories {
            let root = swiftRoot.appendingPathComponent(directory)
            let files = FileManager.default.enumerator(at: root, includingPropertiesForKeys: nil)?
                .compactMap { $0 as? URL }
                .filter { $0.pathExtension == "swift" } ?? []
            for file in files {
                let relative = String(file.path.dropFirst(swiftRoot.path.count + 1))
                let inDesign = relative.contains("/Design/")
                let lines = try String(contentsOf: file, encoding: .utf8).components(separatedBy: "\n")
                for (index, line) in lines.enumerated() {
                    let code = line.components(separatedBy: "//").first ?? line
                    for rule in rules where !(rule.outsideDesignOnly && inDesign) {
                        if code.range(of: rule.pattern, options: .regularExpression) != nil {
                            violations.append("\(relative):\(index + 1): \(rule.reason): \(line.trimmingCharacters(in: .whitespaces))")
                        }
                    }
                }
            }
        }
        return violations
    }

    @Test("Loopflow, LoopflowMac and LoopflowiOS set every font from the ramp")
    func everyFontComesFromTheRamp() throws {
        let violations = try Self.violations(in: ["Loopflow", "LoopflowMac", "LoopflowiOS"], [
            (#"(Font)?\.custom\("#, "a face chosen outside Design/", true),
            (#"\.system\("#, "a system font", false),
            (#"\.font\(\.(largeTitle|title|title2|title3|headline|subheadline|body|callout|footnote|caption|caption2)\b"#, "a system text style", false),
            (#"Typography\.\w+\("#, "a sized Typography call", false),
        ])
        #expect(violations.isEmpty, "\(violations.joined(separator: "\n"))")
    }

    @Test("LoopflowMac draws every empty or failed pane as EmptyState")
    func noGreyIconOverACentredTitle() throws {
        let violations = try Self.violations(in: ["LoopflowMac"], [
            (#"ContentUnavailableView"#, "a centred icon and title; use EmptyState", false),
        ])
        #expect(violations.isEmpty, "\(violations.joined(separator: "\n"))")
    }

    @Test("Loopflow and LoopflowMac take every font size, padding, spacing and radius from the design system")
    func everyMeasureIsNamed() throws {
        let violations = try Self.violations(in: ["Loopflow", "LoopflowMac"], [
            (#"(fixedS|s)ize: *[0-9]"#, "a font size outside the ramp", true),
            (#"\.padding\(([^)]*, *)?-?[0-9]"#, "a padding outside Spacing", false),
            (#"spacing: *[1-9]"#, "a spacing outside Spacing", false),
            (#"[cC]ornerRadius: *[0-9]"#, "a radius outside CornerRadius", true),
        ])
        #expect(violations.isEmpty, "\(violations.joined(separator: "\n"))")
    }

    /// `spacing: 0` passes: it says "no gap", which is not a step on the scale.
    @Test("LoopflowiOS draws buttons, states and spacing from the shared design")
    func thePhoneUsesTheSharedButtonStateViewAndSteps() throws {
        let violations = try Self.violations(in: ["LoopflowiOS"], [
            (#"\.borderedProminent"#, "a hand-made primary button; use LoopflowButtonStyle(.accent)", false),
            (#"ContentUnavailableView"#, "a stock empty view; use StateView", false),
            (#"padding\((\.\w+, *)?\d|EdgeInsets\([^)]*\d"#, "a literal padding; use a Spacing step", false),
            (#"spacing: *[1-9]"#, "a literal spacing; use a Spacing step", false),
            (#"ornerRadius: *\d"#, "a literal radius; use a CornerRadius step", false),
        ])
        #expect(violations.isEmpty, "\(violations.joined(separator: "\n"))")
    }
}
