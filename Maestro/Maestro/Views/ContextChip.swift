// Toggleable context chip - Linear/Notion style.

import SwiftUI

struct ContextChip: View {
    let label: String
    @Binding var isOn: Bool
    var color: Color = .accentColor

    var body: some View {
        Button {
            isOn.toggle()
        } label: {
            Text(label)
                .font(.system(size: 12, weight: .medium))
                .padding(.horizontal, 10)
                .padding(.vertical, 5)
                .background(
                    Capsule()
                        .fill(isOn ? color.opacity(0.15) : Color.primary.opacity(0.05))
                )
                .overlay(
                    Capsule()
                        .stroke(isOn ? color.opacity(0.3) : Color.clear, lineWidth: 1)
                )
                .foregroundStyle(isOn ? color : .secondary)
        }
        .buttonStyle(.plain)
    }
}

struct FileChip: View {
    let name: String
    let icon: String
    var onRemove: () -> Void

    var body: some View {
        HStack(spacing: 4) {
            Image(systemName: icon)
                .font(.system(size: 10))
            Text(name)
                .font(.system(size: 11))
                .lineLimit(1)
            Button {
                onRemove()
            } label: {
                Image(systemName: "xmark")
                    .font(.system(size: 9, weight: .medium))
            }
            .buttonStyle(.plain)
            .opacity(0.6)
        }
        .padding(.horizontal, 8)
        .padding(.vertical, 4)
        .background(Capsule().fill(Color.orange.opacity(0.12)))
        .foregroundStyle(Color.orange)
    }
}

struct AddFileButton: View {
    var action: () -> Void

    var body: some View {
        Button(action: action) {
            Image(systemName: "plus")
                .font(.system(size: 10, weight: .medium))
                .foregroundStyle(.secondary)
                .padding(6)
                .background(Circle().fill(Color.primary.opacity(0.05)))
        }
        .buttonStyle(.plain)
    }
}

#Preview {
    HStack(spacing: 8) {
        ContextChip(label: "Docs", isOn: .constant(true), color: .blue)
        ContextChip(label: "Files", isOn: .constant(true), color: .teal)
        ContextChip(label: "Diff", isOn: .constant(false), color: .green)
        ContextChip(label: "Clipboard", isOn: .constant(false), color: .purple)

        Divider().frame(height: 16)

        FileChip(name: "auth.py", icon: "doc.text") {}
        FileChip(name: "tests/", icon: "folder") {}

        AddFileButton {}

        Spacer()

        Text("14.2k")
            .font(.system(size: 12))
            .foregroundStyle(.secondary)
    }
    .padding()
}
