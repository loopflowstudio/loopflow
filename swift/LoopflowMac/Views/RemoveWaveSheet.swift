// Remove Wave: renders `lf wave remove --preview --json` verbatim, then removes.
// A failure keeps the sheet open with the error and Retry; removal is idempotent,
// so Retry finishes whatever the interrupted run left.

import SwiftUI
import Loopflow

enum RemoveWavePhase: Equatable {
    case loading
    case preview(WaveRemoval)
    case removing(WaveRemoval)
    case failed(preview: WaveRemoval?, message: String)
}

/// The sheet's content for one phase; the owner runs the commands.
struct RemoveWaveSheetContent: View {
    let name: String
    let phase: RemoveWavePhase
    let onRemove: () -> Void
    let onRetry: () -> Void
    let onCancel: () -> Void
    @Environment(\.palette) private var palette

    private var preview: WaveRemoval? {
        switch phase {
        case .loading: nil
        case .preview(let removal), .removing(let removal): removal
        case .failed(let preview, _): preview
        }
    }

    var body: some View {
        VStack(alignment: .leading, spacing: Spacing.lg) {
            Text("Remove \(name)?")
                .font(Typography.sectionTitle(20))
                .foregroundStyle(palette.text)
            if let preview {
                VStack(alignment: .leading, spacing: Spacing.xs) {
                    ForEach(Array(preview.summary.enumerated()), id: \.offset) { _, line in
                        Text(line).font(Typography.body()).foregroundStyle(palette.textSecondary)
                    }
                }.accessibilityIdentifier("remove-wave-summary")
            } else {
                ProgressView().controlSize(.small)
            }
            if case .failed(_, let message) = phase {
                Text(message)
                    .font(Typography.caption())
                    .foregroundStyle(palette.failure)
                    .accessibilityIdentifier("remove-wave-error")
            }
            HStack(spacing: Spacing.md) {
                Spacer()
                Button("Cancel", action: onCancel).buttonStyle(.plain)
                    .foregroundStyle(palette.textSecondary)
                if case .failed = phase {
                    Button("Retry", action: onRetry)
                        .keyboardShortcut(.defaultAction)
                        .accessibilityIdentifier("remove-wave-retry")
                } else {
                    Button("Remove", role: .destructive, action: onRemove)
                        .keyboardShortcut(.defaultAction)
                        .disabled(preview == nil || phase != .preview(preview!))
                        .accessibilityIdentifier("remove-wave-confirm")
                }
            }
        }
        .padding(Spacing.xl)
        .frame(width: 440)
    }
}

/// Owns the preview and removal commands for one Wave.
struct RemoveWaveSheet: View {
    let model: WorkModel
    /// What `lf wave remove` resolves: the Wave's id or address.
    let name: String
    let displayName: String
    let repo: String
    let onRemoved: () -> Void
    let onCancel: () -> Void
    @State private var phase: RemoveWavePhase = .loading

    var body: some View {
        RemoveWaveSheetContent(
            name: displayName, phase: phase, onRemove: remove, onRetry: retry, onCancel: onCancel
        )
        .task { await loadPreview() }
    }

    private func loadPreview() async {
        do {
            phase = .preview(try await model.removeWave(name: name, preview: true, repo: repo))
        } catch {
            phase = .failed(preview: nil, message: error.localizedDescription)
        }
    }

    private func retry() {
        if case .failed(let preview?, _) = phase {
            phase = .preview(preview)
            remove()
        } else {
            phase = .loading
            Task { await loadPreview() }
        }
    }

    private func remove() {
        guard case .preview(let preview) = phase else { return }
        phase = .removing(preview)
        Task {
            do {
                _ = try await model.removeWave(name: name, preview: false, repo: repo)
                onRemoved()
            } catch {
                phase = .failed(preview: preview, message: error.localizedDescription)
            }
        }
    }
}
