// Design system for Concerto
// Based on UI Skills and Vercel Design Guidelines

import SwiftUI

// MARK: - Spacing Scale (4pt base)

enum Spacing {
    static let xxs: CGFloat = 2
    static let xs: CGFloat = 4
    static let sm: CGFloat = 8
    static let md: CGFloat = 12
    static let lg: CGFloat = 16
    static let xl: CGFloat = 20
    static let xxl: CGFloat = 24
    static let xxxl: CGFloat = 32
}

// MARK: - Hit Targets (Vercel: 24px desktop minimum)

enum HitTarget {
    static let minimum: CGFloat = 24
    static let comfortable: CGFloat = 32
    static let touch: CGFloat = 44
}

// MARK: - Z-Index Scale (UI Skills: fixed scale, no arbitrary values)

enum ZIndex {
    static let base: Double = 0
    static let dropdown: Double = 100
    static let modal: Double = 200
    static let toast: Double = 300
    static let tooltip: Double = 400
}

// MARK: - Corner Radius

enum CornerRadius {
    static let sm: CGFloat = 4
    static let md: CGFloat = 8
    static let lg: CGFloat = 12
    static let xl: CGFloat = 16
    static let full: CGFloat = 9999
}

// MARK: - Typography
// Adapted from loopflowstudio/website/TYPOGRAPHY.md
// Serif: Cormorant Garamond - headlines, hero
// Sans: Lato - body, UI
// Mono: JetBrains Mono - code, terminal

enum Typography {
    static let serifFamily = "Cormorant Garamond"
    static let sansFamily = "Lato"
    static let monoFamily = "JetBrains Mono"

    static func heroTitle(_ size: CGFloat = 32) -> Font {
        .custom(serifFamily, size: size).weight(.semibold)
    }

    static func sectionTitle(_ size: CGFloat = 20) -> Font {
        .custom(serifFamily, size: size).weight(.medium)
    }

    static func body(_ size: CGFloat = 14) -> Font {
        .custom(sansFamily, size: size)
    }

    static func caption(_ size: CGFloat = 12) -> Font {
        .custom(sansFamily, size: size)
    }

    static func code(_ size: CGFloat = 13) -> Font {
        .custom(monoFamily, size: size)
    }
}

// MARK: - Animation Helpers

enum DesignAnimation {
    static func standard(_ reduceMotion: Bool) -> Animation? {
        reduceMotion ? nil : .easeInOut(duration: 0.2)
    }

    static func fast(_ reduceMotion: Bool) -> Animation? {
        reduceMotion ? nil : .easeOut(duration: 0.1)
    }

    static func spring(_ reduceMotion: Bool) -> Animation? {
        reduceMotion ? nil : .spring(response: 0.3, dampingFraction: 0.7)
    }
}

// MARK: - Accessibility View Modifiers

extension View {
    func accessibleButton(_ label: String, hint: String? = nil) -> some View {
        self
            .accessibilityLabel(label)
            .accessibilityHint(hint ?? "")
            .accessibilityAddTraits(.isButton)
    }

    func accessibleToggle(_ label: String, isOn: Bool) -> some View {
        self
            .accessibilityLabel(label)
            .accessibilityValue(isOn ? "On" : "Off")
            .accessibilityAddTraits(.isButton)
    }

    func minHitTarget() -> some View {
        self.frame(minWidth: HitTarget.minimum, minHeight: HitTarget.minimum)
    }

    /// Adds a keyboard focus ring overlay when focused.
    func keyboardFocusRing(_ isFocused: Bool, cornerRadius: CGFloat = CornerRadius.md) -> some View {
        self.overlay(
            RoundedRectangle(cornerRadius: cornerRadius)
                .stroke(Color.accentColor, lineWidth: 2)
                .opacity(isFocused ? 1 : 0)
        )
    }
}

// MARK: - Button Styles

struct DarkButtonStyle: ButtonStyle {
    @Environment(\.palette) private var palette

    func makeBody(configuration: Configuration) -> some View {
        configuration.label
            .font(Typography.body())
            .foregroundStyle(Color.loopflowCream)
            .padding(.horizontal, 14)
            .padding(.vertical, 8)
            .background(
                RoundedRectangle(cornerRadius: 8)
                    .fill(configuration.isPressed ? palette.accentHover : palette.accent)
            )
    }
}
