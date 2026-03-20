import Foundation
import AppKit
import Carbon.HIToolbox

enum ShortcutKey: Hashable {
    case character(String)
    case special(NSEvent.SpecialKey)
    case keyCode(UInt16)

    var displayKey: String {
        switch self {
        case .character(let character):
            return character.uppercased()
        case .special(let special):
            return switch special {
            case .upArrow: "↑"
            case .downArrow: "↓"
            case .leftArrow: "←"
            case .rightArrow: "→"
            default: "Key"
            }
        case .keyCode(let keyCode):
            return switch keyCode {
            case ShortcutCatalog.returnKeyCode, ShortcutCatalog.keypadEnterKeyCode:
                "Enter"
            case ShortcutCatalog.escapeKeyCode:
                "Esc"
            case ShortcutCatalog.slashKeyCode:
                "/"
            case ShortcutCatalog.fiveKeyCode:
                "5"
            case ShortcutCatalog.quoteKeyCode:
                "'"
            default:
                "Key"
            }
        }
    }
}

struct ShortcutGesture {
    let key: ShortcutKey
    let modifiers: NSEvent.ModifierFlags
    let allowsRepeat: Bool

    var displayKey: String {
        if key == .character("/") && modifiers == [.shift] {
            return "?"
        }

        var value = ""
        if modifiers.contains(.command) { value += "⌘" }
        if modifiers.contains(.shift) { value += "⇧" }
        if modifiers.contains(.option) { value += "⌥" }
        if modifiers.contains(.control) { value += "⌃" }
        return value + key.displayKey
    }
}

struct ShortcutBinding {
    let gesture: ShortcutGesture
    let action: ShortcutAction
    let label: String
    let category: ShortcutCategory
    let requiresWave: Bool
}

struct ChordBinding {
    let first: ShortcutKey
    let second: ShortcutKey
    let action: ShortcutAction
    let label: String
    let category: ShortcutCategory

    var displayKey: String {
        "\(first.displayKey) \(second.displayKey)"
    }
}

enum ShortcutCategory: String, CaseIterable {
    case navigation = "Navigation"
    case waveActions = "Wave Actions"
    case multiplexer = "Multiplexer"
    case tools = "Tools"
    case tabs = "Tabs"
    case global = "Global"
}

enum ShortcutAction: Hashable {
    // Navigation
    case moveDown
    case moveUp
    case selectFocused
    case goToFirst
    case goToLast

    // Wave actions
    case createWave
    case editName
    case deleteWave
    case retryWave
    case stopWave
    case landWave
    case nextWave

    // Tools
    case openTerminal
    case openIDE
    case openFinder
    case viewPR

    // Multiplexer
    case splitVertical
    case splitHorizontal
    case closePane
    case newShellPane
    case focusNextPane
    case focusPreviousPane
<<<<<<< HEAD
<<<<<<< HEAD
=======
    case snapHalf
    case snapThird
    case snapQuarter
>>>>>>> 55cd605c (lf commit: implement)
=======
>>>>>>> 14032ed8 (Remove checked-in build artifacts and trim multiplexer scaffolding)

    // Tabs
    case switchToCurrentTab
    case switchToRunsTab

    // Global
    case focusSessionComposer
    case openCommandPalette
    case showHelp
}

@MainActor
enum ShortcutCatalog {
    static let slashKeyCode = UInt16(kVK_ANSI_Slash)
    static let returnKeyCode = UInt16(kVK_Return)
    static let keypadEnterKeyCode = UInt16(kVK_ANSI_KeypadEnter)
    static let escapeKeyCode = UInt16(kVK_Escape)
    static let fiveKeyCode = UInt16(kVK_ANSI_5)
    static let quoteKeyCode = UInt16(kVK_ANSI_Quote)
    static let normalizedModifierMask: NSEvent.ModifierFlags = [.shift, .command, .option, .control]

    static let shortcuts: [ShortcutBinding] = [
        // Navigation
        ShortcutBinding(
            gesture: ShortcutGesture(key: .character("j"), modifiers: [], allowsRepeat: true),
            action: .moveDown,
            label: "Move down",
            category: .navigation,
            requiresWave: false
        ),
        ShortcutBinding(
            gesture: ShortcutGesture(key: .character("k"), modifiers: [], allowsRepeat: true),
            action: .moveUp,
            label: "Move up",
            category: .navigation,
            requiresWave: false
        ),
        ShortcutBinding(
            gesture: ShortcutGesture(key: .keyCode(ShortcutCatalog.returnKeyCode), modifiers: [], allowsRepeat: false),
            action: .selectFocused,
            label: "Select wave",
            category: .navigation,
            requiresWave: false
        ),

        // Wave actions
        ShortcutBinding(
            gesture: ShortcutGesture(key: .character("c"), modifiers: [], allowsRepeat: false),
            action: .createWave,
            label: "Create wave",
            category: .waveActions,
            requiresWave: false
        ),
        ShortcutBinding(
            gesture: ShortcutGesture(key: .character("e"), modifiers: [], allowsRepeat: false),
            action: .editName,
            label: "Edit name",
            category: .waveActions,
            requiresWave: true
        ),
        ShortcutBinding(
            gesture: ShortcutGesture(key: .character("d"), modifiers: [], allowsRepeat: false),
            action: .deleteWave,
            label: "Delete wave",
            category: .waveActions,
            requiresWave: true
        ),
        ShortcutBinding(
            gesture: ShortcutGesture(key: .character("r"), modifiers: [], allowsRepeat: false),
            action: .retryWave,
            label: "Retry wave",
            category: .waveActions,
            requiresWave: true
        ),
        ShortcutBinding(
            gesture: ShortcutGesture(key: .character("s"), modifiers: [], allowsRepeat: false),
            action: .stopWave,
            label: "Stop wave",
            category: .waveActions,
            requiresWave: true
        ),
        ShortcutBinding(
            gesture: ShortcutGesture(key: .character("l"), modifiers: [], allowsRepeat: false),
            action: .landWave,
            label: "Land wave",
            category: .waveActions,
            requiresWave: true
        ),
        ShortcutBinding(
            gesture: ShortcutGesture(key: .character("n"), modifiers: [], allowsRepeat: false),
            action: .nextWave,
            label: "Next iteration",
            category: .waveActions,
            requiresWave: true
        ),

        // Tools
        ShortcutBinding(
            gesture: ShortcutGesture(key: .character("t"), modifiers: [], allowsRepeat: false),
            action: .openTerminal,
            label: "Open terminal",
            category: .tools,
            requiresWave: true
        ),
        ShortcutBinding(
            gesture: ShortcutGesture(key: .character("i"), modifiers: [], allowsRepeat: false),
            action: .openIDE,
            label: "Open IDE",
            category: .tools,
            requiresWave: true
        ),
        ShortcutBinding(
            gesture: ShortcutGesture(key: .character("f"), modifiers: [], allowsRepeat: false),
            action: .openFinder,
            label: "Reveal in Finder",
            category: .tools,
            requiresWave: true
        ),
        ShortcutBinding(
            gesture: ShortcutGesture(key: .character("p"), modifiers: [], allowsRepeat: false),
            action: .viewPR,
            label: "View PR",
            category: .tools,
            requiresWave: true
        ),

        // Multiplexer
        ShortcutBinding(
<<<<<<< HEAD
<<<<<<< HEAD
<<<<<<< HEAD
            gesture: ShortcutGesture(key: .character("\\"), modifiers: [.command], allowsRepeat: false),
=======
            gesture: ShortcutGesture(key: .character("d"), modifiers: [.command], allowsRepeat: false),
>>>>>>> 55cd605c (lf commit: implement)
=======
            gesture: ShortcutGesture(key: .character("\\"), modifiers: [.command], allowsRepeat: false),
>>>>>>> d5db82d4 (lf land: stage uncommitted changes)
=======
            gesture: ShortcutGesture(key: .keyCode(ShortcutCatalog.fiveKeyCode), modifiers: [.control, .shift], allowsRepeat: false),
>>>>>>> 0e412996 (concerto: polish workspace keyboard routing and review docs)
            action: .splitVertical,
            label: "Split vertical",
            category: .multiplexer,
            requiresWave: true
        ),
        ShortcutBinding(
<<<<<<< HEAD
<<<<<<< HEAD
<<<<<<< HEAD
            gesture: ShortcutGesture(key: .character("\\"), modifiers: [.command, .shift], allowsRepeat: false),
=======
            gesture: ShortcutGesture(key: .character("d"), modifiers: [.command, .shift], allowsRepeat: false),
>>>>>>> 55cd605c (lf commit: implement)
=======
            gesture: ShortcutGesture(key: .character("\\"), modifiers: [.command, .shift], allowsRepeat: false),
>>>>>>> d5db82d4 (lf land: stage uncommitted changes)
=======
            gesture: ShortcutGesture(key: .keyCode(ShortcutCatalog.quoteKeyCode), modifiers: [.control, .shift], allowsRepeat: false),
>>>>>>> 0e412996 (concerto: polish workspace keyboard routing and review docs)
            action: .splitHorizontal,
            label: "Split horizontal",
            category: .multiplexer,
            requiresWave: true
        ),
        ShortcutBinding(
            gesture: ShortcutGesture(key: .character("w"), modifiers: [.command], allowsRepeat: false),
            action: .closePane,
            label: "Close pane",
            category: .multiplexer,
            requiresWave: true
        ),
        ShortcutBinding(
            gesture: ShortcutGesture(key: .keyCode(ShortcutCatalog.returnKeyCode), modifiers: [.command, .shift], allowsRepeat: false),
            action: .newShellPane,
            label: "New shell",
            category: .multiplexer,
            requiresWave: true
        ),
        ShortcutBinding(
<<<<<<< HEAD
<<<<<<< HEAD
            gesture: ShortcutGesture(key: .special(.rightArrow), modifiers: [.command, .option], allowsRepeat: false),
=======
            gesture: ShortcutGesture(key: .special(.rightArrow), modifiers: [.command], allowsRepeat: false),
>>>>>>> 55cd605c (lf commit: implement)
=======
            gesture: ShortcutGesture(key: .special(.rightArrow), modifiers: [.command, .option], allowsRepeat: false),
>>>>>>> d5db82d4 (lf land: stage uncommitted changes)
            action: .focusNextPane,
            label: "Focus next pane",
            category: .multiplexer,
            requiresWave: true
        ),
        ShortcutBinding(
<<<<<<< HEAD
<<<<<<< HEAD
            gesture: ShortcutGesture(key: .special(.leftArrow), modifiers: [.command, .option], allowsRepeat: false),
=======
            gesture: ShortcutGesture(key: .special(.leftArrow), modifiers: [.command], allowsRepeat: false),
>>>>>>> 55cd605c (lf commit: implement)
=======
            gesture: ShortcutGesture(key: .special(.leftArrow), modifiers: [.command, .option], allowsRepeat: false),
>>>>>>> d5db82d4 (lf land: stage uncommitted changes)
            action: .focusPreviousPane,
            label: "Focus previous pane",
            category: .multiplexer,
            requiresWave: true
        ),

        // Tabs
        ShortcutBinding(
            gesture: ShortcutGesture(key: .character("1"), modifiers: [], allowsRepeat: false),
            action: .switchToCurrentTab,
            label: "Current tab",
            category: .tabs,
            requiresWave: true
        ),
        ShortcutBinding(
            gesture: ShortcutGesture(key: .character("2"), modifiers: [], allowsRepeat: false),
            action: .switchToRunsTab,
            label: "Runs tab",
            category: .tabs,
            requiresWave: true
        ),

        // Global
        ShortcutBinding(
            gesture: ShortcutGesture(key: .character("/"), modifiers: [], allowsRepeat: false),
            action: .focusSessionComposer,
            label: "Focus composer",
            category: .global,
            requiresWave: false
        ),
        ShortcutBinding(
            gesture: ShortcutGesture(key: .character("/"), modifiers: [.shift], allowsRepeat: false),
            action: .showHelp,
            label: "This help",
            category: .global,
            requiresWave: false
        ),
        ShortcutBinding(
            gesture: ShortcutGesture(key: .character("k"), modifiers: [.command], allowsRepeat: false),
            action: .openCommandPalette,
            label: "Command palette",
            category: .global,
            requiresWave: false
        ),
    ]

    static let chords: [ChordBinding] = [
        ChordBinding(
            first: .character("g"),
            second: .character("h"),
            action: .goToFirst,
            label: "Go to first wave",
            category: .navigation
        ),
        ChordBinding(
            first: .character("g"),
            second: .character("l"),
            action: .goToLast,
            label: "Go to last wave",
            category: .navigation
        ),
    ]
}

extension Notification.Name {
    static let toggleCommandPalette = Notification.Name("toggleCommandPalette")
    static let selectPortfolioWave = Notification.Name("selectPortfolioWave")
    static let newWaveRequested = Notification.Name("newWaveRequested")
    static let editWaveName = Notification.Name("editWaveName")
    static let moveFocusDown = Notification.Name("moveFocusDown")
    static let moveFocusUp = Notification.Name("moveFocusUp")
    static let selectFocusedWave = Notification.Name("selectFocusedWave")
    static let goToFirstWave = Notification.Name("goToFirstWave")
    static let goToLastWave = Notification.Name("goToLastWave")
    static let viewWavePR = Notification.Name("viewWavePR")
    static let switchToCurrentTab = Notification.Name("switchToCurrentTab")
    static let switchToRunsTab = Notification.Name("switchToRunsTab")
}
