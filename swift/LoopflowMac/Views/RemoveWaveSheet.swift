// Remove Wave: renders `lf wave remove --preview --json` verbatim, then removes.
// A failure keeps the sheet open with the error and Retry; removal is idempotent,
// so Retry finishes whatever the interrupted run left. A receipt with leftovers
// (an Initiative still live, uncommitted files, prose mentions) stays open with
// the same lines the CLI prints.

import SwiftUI
import Loopflow

enum RemoveWavePhase: Equatable {
    case loading
    case preview(WaveRemoval)
    case removing(WaveRemoval)
    case failed(preview: WaveRemoval?, message: String)
    /// Removed, with `summary` lines a person still has to act on.
    case leftovers(WaveRemoval)
}

/// The sheet's content for one phase; the owner runs the commands.
struct RemoveWaveSheetContent: View {
    let name: String
    let phase: RemoveWavePhase
    let onRemove: () -> Void
    let onRetry: () -> Void
    let onCancel: () -> Void
    let onDone: () -> Void
    @Environment(\.palette) private var palette

    private var removal: WaveRemoval? {
        switch phase {
        case .loading: nil
        case .preview(let removal), .removing(let removal), .leftovers(let removal): removal
        case .failed(let preview, _): preview
        }
    }

    private var removed: Bool {
        if case .leftovers = phase { true } else { false }
    }

    private var canRemove: Bool {
        if case .preview = phase { true } else { false }
    }

    var body: some View {
        VStack(alignment: .leading, spacing: Spacing.lg) {
            Text(removed ? "\(name) removed" : "Remove \(name)?")
                .font(Typography.sectionTitle(20))
                .foregroundStyle(palette.text)
            if let removal {
                VStack(alignment: .leading, spacing: Spacing.xs) {
                    ForEach(Array(removal.summary.enumerated()), id: \.offset) { _, line in
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
                if removed {
                    Button("Done", action: onDone)
                        .keyboardShortcut(.defaultAction)
                        .accessibilityIdentifier("remove-wave-done")
                } else if case .failed = phase {
                    Button("Cancel", action: onCancel).buttonStyle(.plain)
                        .foregroundStyle(palette.textSecondary)
                    Button("Retry", action: onRetry)
                        .keyboardShortcut(.defaultAction)
                        .accessibilityIdentifier("remove-wave-retry")
                } else {
                    Button("Cancel", action: onCancel).buttonStyle(.plain)
                        .foregroundStyle(palette.textSecondary)
                    Button("Remove", role: .destructive, action: onRemove)
                        .keyboardShortcut(.defaultAction)
                        .disabled(!canRemove)
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
            name: displayName, phase: phase, onRemove: remove, onRetry: retry, onCancel: onCancel,
            onDone: onRemoved
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
                let receipt = try await model.removeWave(name: name, preview: false, repo: repo)
                if receipt.hasLeftovers {
                    phase = .leftovers(receipt)
                } else {
                    onRemoved()
                }
            } catch {
                phase = .failed(preview: preview, message: error.localizedDescription)
            }
        }
    }
}
