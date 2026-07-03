// TypeaheadComponents - shared building blocks for fish-style typeahead fields.

import SwiftUI
import LoopflowCore
import AppKit

// MARK: - Ghost Text Field (Fish-style autocomplete)

struct GhostTextField: NSViewRepresentable {
    @Binding var text: String
    let ghost: String
    let placeholder: String
    let onTab: () -> Void
    let onEnter: () -> Void
    let onBackspace: () -> Void
    var onFocusChange: ((Bool) -> Void)? = nil

    func makeNSView(context: Context) -> NSTextField {
        let field = NSTextField()
        field.isBordered = false
        field.drawsBackground = false
        field.focusRingType = .none
        field.placeholderString = placeholder
        field.delegate = context.coordinator
        field.font = .monospacedSystemFont(ofSize: 13, weight: .regular)
        return field
    }

    func updateNSView(_ nsView: NSTextField, context: Context) {
        // Track ghost so coordinator can strip it from typed input
        context.coordinator.currentGhost = ghost

        if nsView.stringValue != text {
            nsView.stringValue = text
        }

        // Build attributed string with ghost text
        let font = NSFont.monospacedSystemFont(ofSize: 13, weight: .regular)
        let fullText = NSMutableAttributedString(
            string: text,
            attributes: [.foregroundColor: NSColor.labelColor, .font: font]
        )

        if !ghost.isEmpty && !text.isEmpty {
            let ghostPart = NSAttributedString(
                string: ghost,
                attributes: [.foregroundColor: NSColor.placeholderTextColor, .font: font]
            )
            fullText.append(ghostPart)
        }

        // When editing, the field editor (NSTextView) owns the text — update it directly
        if let editor = nsView.currentEditor() as? NSTextView,
           editor.string != fullText.string {
            editor.textStorage?.setAttributedString(fullText)
            editor.setSelectedRange(NSRange(location: text.count, length: 0))
        } else if nsView.attributedStringValue.string != fullText.string {
            nsView.attributedStringValue = fullText
        }
    }

    func makeCoordinator() -> Coordinator {
        Coordinator(self)
    }

    class Coordinator: NSObject, NSTextFieldDelegate {
        var parent: GhostTextField
        var currentGhost: String = ""

        init(_ parent: GhostTextField) {
            self.parent = parent
        }

        func controlTextDidBeginEditing(_ obj: Notification) {
            parent.onFocusChange?(true)
        }

        func controlTextDidEndEditing(_ obj: Notification) {
            parent.onFocusChange?(false)
        }

        func controlTextDidChange(_ obj: Notification) {
            guard let field = obj.object as? NSTextField else { return }
            var value = field.stringValue
            // Ghost text is part of the attributed string, so typing inserts
            // characters into the middle (e.g. "s" + ghost "cripts" = "scripts",
            // then typing "w" at cursor produces "swcripts"). Strip the ghost suffix.
            if !currentGhost.isEmpty && value.hasSuffix(currentGhost) {
                value = String(value.dropLast(currentGhost.count))
            }
            parent.text = value
        }

        func control(_ control: NSControl, textView: NSTextView, doCommandBy commandSelector: Selector) -> Bool {
            if commandSelector == #selector(NSResponder.insertTab(_:)) {
                parent.onTab()
                return true
            }
            if commandSelector == #selector(NSResponder.insertNewline(_:)) {
                parent.onEnter()
                return true
            }
            if commandSelector == #selector(NSResponder.deleteBackward(_:)) {
                if parent.text.isEmpty {
                    parent.onBackspace()
                    return true
                }
            }
            return false
        }
    }
}

// MARK: - Typeahead Chip

struct TypeaheadChip: View {
    let icon: String
    let label: String
    let helpText: String
    let onRemove: () -> Void

    @Environment(\.palette) private var palette
    @State private var isHovered = false

    var body: some View {
        HStack(spacing: 4) {
            Image(systemName: icon)
                .font(Typography.caption(10))
                .foregroundStyle(palette.accent)

            Text(label)
                .font(Typography.code(12))
                .lineLimit(1)

            Button {
                onRemove()
            } label: {
                Image(systemName: "xmark")
                    .font(Typography.caption(9)).fontWeight(.semibold)
                    .foregroundStyle(palette.textSecondary)
            }
            .buttonStyle(.plain)
            .opacity(isHovered ? 1 : 0.6)
        }
        .padding(.horizontal, 8)
        .padding(.vertical, 4)
        .background(palette.accent.opacity(0.15))
        .clipShape(RoundedRectangle(cornerRadius: CornerRadius.sm))
        .onHover { isHovered = $0 }
        .help(helpText)
    }
}

// MARK: - Wrapping Horizontal Layout

struct WrappingHStack: Layout {
    var spacing: CGFloat = 4

    func sizeThatFits(proposal: ProposedViewSize, subviews: Subviews, cache: inout ()) -> CGSize {
        let rows = computeRows(proposal: proposal, subviews: subviews)
        var height: CGFloat = 0
        for (index, row) in rows.enumerated() {
            let rowHeight = row.map { $0.sizeThatFits(.unspecified).height }.max() ?? 0
            height += rowHeight
            if index < rows.count - 1 { height += spacing }
        }
        return CGSize(width: proposal.width ?? 0, height: height)
    }

    func placeSubviews(in bounds: CGRect, proposal: ProposedViewSize, subviews: Subviews, cache: inout ()) {
        let rows = computeRows(proposal: proposal, subviews: subviews)
        var y = bounds.minY
        for row in rows {
            let rowHeight = row.map { $0.sizeThatFits(.unspecified).height }.max() ?? 0
            var x = bounds.minX
            for subview in row {
                let size = subview.sizeThatFits(.unspecified)
                subview.place(at: CGPoint(x: x, y: y), proposal: ProposedViewSize(size))
                x += size.width + spacing
            }
            y += rowHeight + spacing
        }
    }

    private func computeRows(proposal: ProposedViewSize, subviews: Subviews) -> [[LayoutSubviews.Element]] {
        let maxWidth = proposal.width ?? .infinity
        var rows: [[LayoutSubviews.Element]] = [[]]
        var currentWidth: CGFloat = 0

        for subview in subviews {
            let size = subview.sizeThatFits(.unspecified)
            if currentWidth + size.width > maxWidth && !rows[rows.count - 1].isEmpty {
                rows.append([])
                currentWidth = 0
            }
            rows[rows.count - 1].append(subview)
            currentWidth += size.width + spacing
        }
        return rows
    }
}

