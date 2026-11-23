import Foundation

/// A Machine's `theme:` in its `~/.lf/config.yaml`, in Ghostty's value syntax: `light:A,dark:B`
/// or one name. Every surface reads it through that Machine's server.
public enum ThemeChoice: Equatable, Sendable {
    case single(String)
    case pair(light: String?, dark: String?)

    public static func parse(_ text: String) -> Self? {
        let text = text.trimmingCharacters(in: .whitespaces)
        guard !text.isEmpty else { return nil }
        var light: String?, dark: String?
        for part in text.split(separator: ",") {
            let pair = part.split(separator: ":", maxSplits: 1).map { $0.trimmingCharacters(in: .whitespaces) }
            switch (pair.first, pair.count) {
            case ("light", 2): light = pair[1]
            case ("dark", 2): dark = pair[1]
            default: return .single(text)
            }
        }
        return .pair(light: light, dark: dark)
    }

    /// What to draw in `dark` or light appearance. One name is that theme in either; a name
    /// `resolve` doesn't know draws Loopflow's own theme for the side.
    public func theme(dark: Bool, resolve: (String) -> ThemeDefinition?) -> ThemeDefinition {
        let name: String? = switch self {
        case .single(let name): name
        case .pair(let light, let other): dark ? other : light
        }
        return name.flatMap(resolve) ?? (dark ? .dark : .light)
    }
}
