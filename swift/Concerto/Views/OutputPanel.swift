// Live output panel showing streaming task output.

import SwiftUI
import LoopflowCore

struct OutputPanel: View {
    @Bindable var appState: AppState
    @State private var isExpanded = false
    @State private var selectedSessionId: String?
    @Environment(\.colorScheme) private var colorScheme
    @Environment(\.accessibilityReduceMotion) private var reduceMotion

    private var palette: LoopflowPalette {
        LoopflowPalette.make(for: colorScheme)
    }

    var body: some View {
        VStack(spacing: 0) {
            // Header bar - always visible when there's activity
            if !appState.activeSessionIds.isEmpty || hasRecentOutput {
                outputHeader
            }

            // Expandable output area
            if isExpanded, let sessionId = effectiveSessionId {
                outputContent(sessionId: sessionId)
            }
        }
    }

    private var effectiveSessionId: String? {
        selectedSessionId ?? appState.activeSessionIds.first ?? appState.liveOutputBySession.keys.first
    }

    private var hasRecentOutput: Bool {
        !appState.liveOutputBySession.isEmpty
    }

    private var outputHeader: some View {
        HStack {
            // Session picker if multiple
            if appState.liveOutputBySession.count > 1 {
                Picker("Session", selection: $selectedSessionId) {
                    ForEach(Array(appState.liveOutputBySession.keys.sorted()), id: \.self) { id in
                        HStack {
                            if appState.activeSessionIds.contains(id) {
                                Circle()
                                    .fill(.green)
                                    .frame(width: 6, height: 6)
                            }
                            Text(String(id.prefix(8)))
                        }
                        .tag(id as String?)
                    }
                }
                .labelsHidden()
                .frame(maxWidth: 100)
            }

            // Activity indicator
            if !appState.activeSessionIds.isEmpty {
                Circle()
                    .fill(.green)
                    .frame(width: 8, height: 8)

                Text("\(appState.activeSessionIds.count) running")
                    .font(.caption)
                    .foregroundStyle(.secondary)
            } else {
                Circle()
                    .fill(.gray)
                    .frame(width: 8, height: 8)

                Text("idle")
                    .font(.caption)
                    .foregroundStyle(.secondary)
            }

            Spacer()

            // Line count
            if let sessionId = effectiveSessionId,
               let lineCount = appState.liveOutputBySession[sessionId]?.count {
                Text("\(lineCount) lines")
                    .font(.caption)
                    .foregroundStyle(.tertiary)
            }

            // Clear button
            if hasRecentOutput {
                Button {
                    clearOutput()
                } label: {
                    Image(systemName: "trash")
                        .font(.caption)
                }
                .buttonStyle(.plain)
                .help("Clear output")
            }

            // Expand/collapse toggle
            Button {
                withAnimation(DesignAnimation.standard(reduceMotion)) {
                    isExpanded.toggle()
                }
            } label: {
                Image(systemName: isExpanded ? "chevron.down" : "chevron.up")
                    .font(.caption)
            }
            .buttonStyle(.plain)
            .help(isExpanded ? "Collapse" : "Expand")
            .accessibleButton("Toggle output panel", hint: isExpanded ? "Collapse" : "Expand")
            .minHitTarget()
        }
        .padding(.horizontal, 16)
        .padding(.vertical, 8)
        .background(palette.surface)
    }

    @ViewBuilder
    private func outputContent(sessionId: String) -> some View {
        ScrollViewReader { proxy in
            ScrollView {
                LazyVStack(alignment: .leading, spacing: 2) {
                    ForEach(appState.liveOutputBySession[sessionId] ?? []) { line in
                        Text(line.text)
                            .font(.system(.caption, design: .monospaced))
                            .foregroundStyle(colorFor(line.text))
                            .textSelection(.enabled)
                            .id(line.id)
                    }
                }
                .padding(.horizontal, 16)
                .padding(.vertical, 8)
                .frame(maxWidth: .infinity, alignment: .leading)
            }
            .frame(height: 200)
            .background(palette.surface)
            .onChange(of: appState.liveOutputBySession[sessionId]?.count) { _, _ in
                // Auto-scroll to bottom
                if let lastLine = appState.liveOutputBySession[sessionId]?.last {
                    withAnimation(DesignAnimation.fast(reduceMotion)) {
                        proxy.scrollTo(lastLine.id, anchor: .bottom)
                    }
                }
            }
        }
    }

    private func colorFor(_ text: String) -> Color {
        if text.hasPrefix("→") { return .blue }
        if text.hasPrefix("✓") { return .green }
        if text.hasPrefix("✗") { return .red }
        return .primary
    }

    private func clearOutput() {
        // Clear output for sessions that are no longer active
        for sessionId in appState.liveOutputBySession.keys {
            if !appState.activeSessionIds.contains(sessionId) {
                appState.liveOutputBySession.removeValue(forKey: sessionId)
            }
        }
        // If nothing left, also collapse
        if appState.liveOutputBySession.isEmpty {
            isExpanded = false
        }
    }
}

#Preview {
    let state = AppState()
    return OutputPanel(appState: state)
        .frame(width: 600)
}
