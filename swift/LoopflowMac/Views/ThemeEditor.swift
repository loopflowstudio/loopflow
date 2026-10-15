import AppKit
import SwiftUI
import Loopflow

struct ThemeEditor: View {
    let appearance: AppearanceStore
    @Environment(\.palette) private var palette

    var body: some View {
        VStack(alignment: .leading, spacing: 16) {
            TextField("Theme name", text: Binding(get: { appearance.selectedRecord.name }, set: { name in
                appearance.editTheme { $0.name = name }
            }))
            .textFieldStyle(.roundedBorder)
            .accessibilityIdentifier("appearance-theme-name")
            Toggle("Dark appearance for native controls", isOn: Binding(get: { appearance.selectedRecord.palette.isDark }, set: { dark in
                appearance.editTheme { $0.palette.isDark = dark }
            }))
            colors("Surfaces", fields: [
                ("Background", \.background), ("Surface", \.surface), ("Muted surface", \.surfaceMuted),
                ("Raised surface", \.surfaceElement), ("Border", \.border), ("Strong border", \.borderStrong),
                ("Active border", \.borderActive), ("Shadow", \.shadow),
            ])
            colors("Text", fields: [
                ("Body", \.text), ("Secondary", \.textSecondary), ("Tertiary", \.textTertiary), ("Headings", \.emphasisInk),
            ])
            colors("Actions and focus", fields: [
                ("Button", \.accent), ("Button hover", \.accentHover), ("Button pressed", \.accentPressed),
                ("Button text", \.accentText), ("Action text and focus", \.accentInk),
            ])
            colors("Work selection", fields: [
                ("Selected fill", \.workSelectedFill), ("Selected text", \.workSelectedText),
                ("Selected secondary text", \.workSelectedSecondaryText), ("File selection tint", \.selectionFill.rgb),
            ])
            HStack {
                Text("File selection opacity").foregroundStyle(palette.textSecondary)
                Slider(value: Binding(get: { appearance.selectedRecord.palette.selectionFill.opacity }, set: { value in
                    appearance.editTheme { $0.palette.selectionFill.opacity = value }
                }), in: 0...1)
            }
            colors("Activity", fields: [
                ("Working text", \.workingTone.inkRGB), ("Working fill", \.workingTone.fillRGB),
                ("Waiting text", \.waitingTone.inkRGB), ("Waiting fill", \.waitingTone.fillRGB),
                ("Error text", \.failureTone.inkRGB), ("Error fill", \.failureTone.fillRGB),
                ("Success text", \.successTone.inkRGB), ("Success fill", \.successTone.fillRGB),
                ("Stopped text", \.stoppedTone.inkRGB), ("Stopped fill", \.stoppedTone.fillRGB),
            ])
            DisclosureGroup("Terminal") {
                colorGrid {
                    ForEach(Array(terminalFields.enumerated()), id: \.offset) { _, field in
                        ThemeColorPicker(title: field.0, rgb: Binding(get: {
                            appearance.selectedRecord.terminal[keyPath: field.1]
                        }, set: { value in appearance.editTheme { $0.terminal[keyPath: field.1] = value } }))
                    }
                }
                DisclosureGroup("ANSI colors (0–15)") { indexedColors(0..<16) }
                DisclosureGroup("Extended colors (16–255)") { indexedColors(16..<256) }
            }
            Text("Reset restores this slot’s original \(appearance.originalTheme.name) name and colors. Apply saves the whole library; Cancel discards your edits.")
                .font(.caption).foregroundStyle(palette.textSecondary)
        }
        .accessibilityIdentifier("appearance-theme-editor")
    }

    private let terminalFields: [(String, WritableKeyPath<TerminalPalette, ThemeRGB>)] = [
        ("Background", \.background), ("Foreground", \.foreground), ("Cursor", \.cursor),
        ("Cursor text", \.cursorText), ("Selection", \.selectionBackground), ("Selection text", \.selectionForeground),
    ]

    private func colors(_ title: String, fields: [(String, WritableKeyPath<PaletteDefinition, ThemeRGB>)]) -> some View {
        DisclosureGroup(title) {
            colorGrid {
                ForEach(Array(fields.enumerated()), id: \.offset) { _, field in
                    ThemeColorPicker(title: field.0, rgb: Binding(get: {
                        appearance.selectedRecord.palette[keyPath: field.1]
                    }, set: { value in appearance.editTheme { $0.palette[keyPath: field.1] = value } }))
                }
            }
        }
    }

    private func indexedColors(_ indices: Range<Int>) -> some View {
        colorGrid {
            ForEach(indices, id: \.self) { index in
                ThemeColorPicker(title: "ANSI \(index)", rgb: Binding(get: {
                    appearance.selectedRecord.terminal.indexed[index]
                }, set: { value in appearance.editTheme { $0.terminal.indexed[index] = value } }))
            }
        }
    }

    private func colorGrid<Content: View>(@ViewBuilder content: () -> Content) -> some View {
        LazyVGrid(columns: [GridItem(.flexible()), GridItem(.flexible())], alignment: .leading, spacing: 10, content: content)
            .padding(.vertical, 8)
    }
}

private struct ThemeColorPicker: View {
    let title: String
    @Binding var rgb: ThemeRGB
    var body: some View {
        HStack {
            ColorPicker(title, selection: Binding(get: { rgb.color }, set: { color in
                guard let color = NSColor(color).usingColorSpace(.sRGB) else { return }
                func channel(_ value: CGFloat) -> UInt32 {
                    UInt32((min(1, max(0, value)) * 255).rounded())
                }
                rgb = ThemeRGB(channel(color.redComponent) << 16 | channel(color.greenComponent) << 8 | channel(color.blueComponent))
            }), supportsOpacity: false)
            Text(rgb.css).font(Typography.code(11)).textSelection(.enabled)
        }
    }
}
