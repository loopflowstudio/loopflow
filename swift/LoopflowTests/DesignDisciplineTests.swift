import Foundation
import Testing

/// Mac text is set on the ramp, and padding, spacing and corner radius on the
/// scale, both in `Loopflow/Design/DesignSystem.swift`. A size, a system text
/// style or a number picked at a call site puts something on screen the design
/// does not have.
@Suite("Design discipline")
struct DesignDisciplineTests {
    private static let swiftRoot = URL(fileURLWithPath: #filePath)
        .deletingLastPathComponent()
        .deletingLastPathComponent()

    private static let rules: [(pattern: String, reason: String, outsideDesignOnly: Bool)] = [
        (#"(Font)?\.custom\("#, "a face chosen outside Design/", true),
        (#"\.system\("#, "a system font", false),
        (#"\.font\(\.(largeTitle|title|title2|title3|headline|subheadline|body|callout|footnote|caption|caption2)\b"#, "a system text style", false),
        (#"Typography\.\w+\("#, "a sized Typography call", false),
        (#"(fixedS|s)ize: *[0-9]"#, "a font size outside the ramp", true),
        (#"\.padding\(([^)]*, *)?-?[0-9]"#, "a padding outside Spacing", false),
        (#"spacing: *[1-9]"#, "a spacing outside Spacing", false),
        (#"[cC]ornerRadius: *[0-9]"#, "a radius outside CornerRadius", true),
    ]

    @Test("Loopflow and LoopflowMac take every font, padding, spacing and radius from the design system")
    func everyFontAndMeasureIsNamed() throws {
        var violations: [String] = []
        for directory in ["Loopflow", "LoopflowMac"] {
            let root = Self.swiftRoot.appendingPathComponent(directory)
            let files = FileManager.default.enumerator(at: root, includingPropertiesForKeys: nil)?
                .compactMap { $0 as? URL }
                .filter { $0.pathExtension == "swift" } ?? []
            for file in files {
                let relative = String(file.path.dropFirst(Self.swiftRoot.path.count + 1))
                let inDesign = relative.contains("/Design/")
                let lines = try String(contentsOf: file, encoding: .utf8).components(separatedBy: "\n")
                for (index, line) in lines.enumerated() {
                    let code = line.components(separatedBy: "//").first ?? line
                    for rule in Self.rules where !(rule.outsideDesignOnly && inDesign) {
                        if code.range(of: rule.pattern, options: .regularExpression) != nil {
                            violations.append("\(relative):\(index + 1): \(rule.reason): \(line.trimmingCharacters(in: .whitespaces))")
                        }
                    }
                }
            }
        }
        #expect(violations.isEmpty, "\(violations.joined(separator: "\n"))")
    }
}
