import Foundation
import Testing

/// Mac text is set on the ramp in `Design/DesignSystem.swift`. A size picked at a
/// call site, a system text style or a sized helper puts a face on screen the
/// design does not have.
@Suite("Typography discipline")
struct TypographyDisciplineTests {
    private static let swiftRoot = URL(fileURLWithPath: #filePath)
        .deletingLastPathComponent()
        .deletingLastPathComponent()

    private static let rules: [(pattern: String, reason: String, outsideDesignOnly: Bool)] = [
        (#"(Font)?\.custom\("#, "a face chosen outside Design/", true),
        (#"\.system\("#, "a system font", false),
        (#"\.font\(\.(largeTitle|title|title2|title3|headline|subheadline|body|callout|footnote|caption|caption2)\b"#, "a system text style", false),
        (#"Typography\.\w+\("#, "a sized Typography call", false),
    ]

    /// Every line of code under `directories` that matches a rule, comments left out.
    private static func violations(in directories: [String],
                                   rules: [(pattern: String, reason: String, outsideDesignOnly: Bool)]) throws -> [String] {
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

    @Test("Loopflow and LoopflowMac set every font from the ramp")
    func everyFontComesFromTheRamp() throws {
        let violations = try Self.violations(in: ["Loopflow", "LoopflowMac"], rules: Self.rules)
        #expect(violations.isEmpty, "\(violations.joined(separator: "\n"))")
    }

    @Test("LoopflowMac draws every empty or failed pane as EmptyState")
    func noGreyIconOverACentredTitle() throws {
        let violations = try Self.violations(in: ["LoopflowMac"], rules: [
            (#"ContentUnavailableView"#, "a centred icon and title; use EmptyState", false),
        ])
        #expect(violations.isEmpty, "\(violations.joined(separator: "\n"))")
    }
}
