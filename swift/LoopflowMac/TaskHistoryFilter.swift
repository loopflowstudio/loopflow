import AppKit
import Foundation
import Loopflow
import SwiftUI

struct TaskHistoryFilter: Equatable {
    var showCompleted = false
    private(set) var days = 7
    private(set) var validationMessage: String?

    mutating func editDays(_ text: String) {
        let text = text.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !text.isEmpty, text.utf8.allSatisfy({ (48...57).contains($0) }),
              let value = Int(text) else {
            validationMessage = "Enter a whole number of 0 or more. Previous range kept."
            return
        }
        days = value
        validationMessage = nil
    }

    mutating func clearValidation() { validationMessage = nil }

    func includes(_ task: TaskPlanningSnapshot, condition: TaskConditionSnapshot, now: Date) -> Bool {
        if !task.isTerminal || condition.unresolvedExecution { return true }
        guard task.isSuccessful, showCompleted else { return false }
        if days == 0 { return true }
        guard let timestamp = task.completedAt, let date = Self.completionDate(timestamp) else { return false }
        return date <= now && date >= now.addingTimeInterval(-Double(days) * 86_400)
    }

    private static func completionDate(_ timestamp: String) -> Date? {
        if let date = try? Date.ISO8601FormatStyle(includingFractionalSeconds: true).parse(timestamp) { return date }
        return try? Date.ISO8601FormatStyle().parse(timestamp)
    }
}

struct TaskHistoryControls: View {
    @Binding var filter: TaskHistoryFilter
    @Environment(\.palette) private var palette
    @State private var hovered = false

    var body: some View {
        VStack(alignment: .trailing, spacing: 6) {
            TaskHistoryInlineControl(filter: $filter, color: NSColor(filter.showCompleted ? palette.accentInk : palette.textSecondary))
                .frame(width: 112, height: 26)
                .background(hovered ? palette.surfaceMuted : .clear, in: RoundedRectangle(cornerRadius: 4))
                .onHover { hovered = $0 }
                .help("Include completed Tasks. Click the range to edit; 0 includes all ages.")
            if let message = filter.validationMessage {
                Text(message)
                    .font(Typography.caption())
                    .foregroundStyle(Color.statusWarning)
            }
        }
    }
}

struct TaskHistoryInlineControl: NSViewRepresentable {
    @Binding var filter: TaskHistoryFilter
    let color: NSColor

    func makeNSView(context: Context) -> TaskHistoryInlineView {
        TaskHistoryInlineView(filter: filter, onChange: { filter = $0 })
    }

    func updateNSView(_ view: TaskHistoryInlineView, context: Context) {
        view.onChange = { filter = $0 }
        view.update(filter, color: color)
    }
}

/// One text surface stays in place while editing is enabled or disabled.
final class TaskHistoryInlineView: NSView, NSTextViewDelegate {
    let checkbox = NSButton(checkboxWithTitle: "", target: nil, action: nil)
    let number = HistoryRangeTextView()
    let suffix = HistoryRangeTextView()
    let numberViewport = NSScrollView()
    private(set) var filter: TaskHistoryFilter
    private(set) var editing = false
    var onChange: (TaskHistoryFilter) -> Void

    init(filter: TaskHistoryFilter, onChange: @escaping (TaskHistoryFilter) -> Void) {
        self.filter = filter
        self.onChange = onChange
        super.init(frame: NSRect(x: 0, y: 0, width: 112, height: 26))
        checkbox.setAccessibilityLabel("Include completed Tasks")
        checkbox.setAccessibilityIdentifier("task-history-show")
        checkbox.controlSize = .small
        checkbox.target = self
        checkbox.action = #selector(toggleHistory)
        for text in [number, suffix] {
            text.drawsBackground = false
            text.isRichText = false
            text.isFieldEditor = true
            text.isVerticallyResizable = false
            text.isEditable = false
            text.isSelectable = false
            text.font = NSFont.monospacedDigitSystemFont(ofSize: 12, weight: .regular)
            text.textContainerInset = .zero
            text.textContainer?.lineFragmentPadding = 0
            text.activate = { [weak self] in self?.activateLabel() }
        }
        number.delegate = self
        number.didBlur = { [weak self] in self?.finishEditing() }
        number.setAccessibilityIdentifier("task-history-days")
        number.isHorizontallyResizable = true
        number.textContainer?.widthTracksTextView = false
        number.textContainer?.containerSize = NSSize(width: CGFloat.greatestFiniteMagnitude, height: 16)
        numberViewport.drawsBackground = false
        numberViewport.borderType = .noBorder
        numberViewport.documentView = number
        suffix.string = "Days"
        suffix.participatesInKeyLoop = false
        suffix.setAccessibilityElement(false)
        addSubview(checkbox)
        addSubview(numberViewport)
        addSubview(suffix)
        update(filter, color: .secondaryLabelColor)
    }

    required init?(coder: NSCoder) { fatalError("init(coder:) has not been implemented") }

    func update(_ filter: TaskHistoryFilter, color: NSColor) {
        self.filter = filter
        checkbox.state = filter.showCompleted ? .on : .off
        checkbox.contentTintColor = color
        number.textColor = color
        suffix.textColor = color
        number.isEditable = editing
        number.isSelectable = editing
        if !editing {
            number.string = !filter.showCompleted ? "Completed" : filter.days == 0 ? "All Tasks" : String(filter.days)
            numberViewport.contentView.scroll(to: .zero)
        }
        suffix.isHidden = !editing && (!filter.showCompleted || filter.days == 0)
        number.setAccessibilityRole(editing ? .textField : .button)
        number.setAccessibilityLabel(editing ? "Completion range in days" : filter.showCompleted ? "Edit completed Task range: \(number.string)\(suffix.isHidden ? "" : " Days")" : "Include completed Tasks")
        needsLayout = true
        needsDisplay = true
    }

    override func draw(_ dirtyRect: NSRect) {
        super.draw(dirtyRect)
        if editing {
            (number.textColor ?? .labelColor).setFill()
            NSRect(x: numberViewport.frame.minX, y: numberViewport.frame.minY, width: numberViewport.frame.width, height: 1).fill()
        }
    }

    override func layout() {
        super.layout()
        checkbox.frame = NSRect(x: 6, y: 6, width: 14, height: 14)
        let width = (number.string as NSString).size(withAttributes: [.font: number.font ?? .systemFont(ofSize: 12)]).width.rounded(.up)
        // Long numbers scroll inside the same compact label.
        let fieldWidth = suffix.isHidden ? 79 : min(max(width, 8), 46)
        numberViewport.frame = NSRect(x: 26, y: 5, width: fieldWidth, height: 16)
        number.frame = NSRect(x: 0, y: 0, width: max(fieldWidth, width), height: 16)
        suffix.frame = NSRect(x: 26 + fieldWidth + 5, y: 5, width: 33, height: 16)
    }

    func activateLabel() {
        if !filter.showCompleted {
            filter.showCompleted = true
            filter.clearValidation()
            publish()
            return
        }
        guard !editing else { return }
        editing = true
        filter.clearValidation()
        number.string = String(filter.days)
        publish()
        layoutSubtreeIfNeeded()
        window?.makeFirstResponder(number)
        number.setSelectedRange(NSRange(location: 0, length: number.string.utf16.count))
    }

    @objc func toggleHistory() {
        let enabled = checkbox.state == .on
        finishEditing()
        filter.showCompleted = enabled
        filter.clearValidation()
        publish()
    }

    func finishEditing(cancel: Bool = false) {
        guard editing else { return }
        editing = false
        if cancel { filter.clearValidation() } else { filter.editDays(number.string) }
        publish()
    }

    private func publish() {
        update(filter, color: number.textColor ?? .labelColor)
        onChange(filter)
    }

    func textDidChange(_ notification: Notification) {
        needsLayout = true
        needsDisplay = true
    }

    func textView(_ textView: NSTextView, doCommandBy selector: Selector) -> Bool {
        switch selector {
        case #selector(NSResponder.insertNewline(_:)): finishEditing()
        case #selector(NSResponder.cancelOperation(_:)): finishEditing(cancel: true)
        case #selector(NSResponder.insertTab(_:)):
            finishEditing()
            window?.selectNextKeyView(number)
        case #selector(NSResponder.insertBacktab(_:)):
            finishEditing()
            window?.selectPreviousKeyView(number)
        default: return false
        }
        return true
    }
}

final class HistoryRangeTextView: NSTextView {
    var activate: (() -> Void)?
    var didBlur: (() -> Void)?
    var participatesInKeyLoop = true

    override var acceptsFirstResponder: Bool { participatesInKeyLoop }
    override var canBecomeKeyView: Bool { participatesInKeyLoop }
    override func resignFirstResponder() -> Bool {
        let resigned = super.resignFirstResponder()
        if resigned { didBlur?() }
        return resigned
    }
    override func mouseDown(with event: NSEvent) {
        if isEditable { super.mouseDown(with: event) } else { activate?() }
    }
    override func keyDown(with event: NSEvent) {
        if !isEditable, event.keyCode == 48 {
            if event.modifierFlags.contains(.shift) { window?.selectPreviousKeyView(self) }
            else { window?.selectNextKeyView(self) }
        } else if !isEditable, event.characters == " " || event.characters == "\r" {
            activate?()
        } else { super.keyDown(with: event) }
    }
    override func accessibilityPerformPress() -> Bool {
        activate?()
        return true
    }
}
