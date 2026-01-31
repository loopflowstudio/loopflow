// DirectionPills - inline direction editing with pills.

import SwiftUI
import LoopflowCore

struct DirectionPills: View {
    let wave: Wave

    @Environment(RepoState.self) private var repoState
    @Environment(\.colorScheme) private var colorScheme

    @State private var showingPicker = false
    @State private var isSaving = false

    private var palette: LoopflowPalette { LoopflowPalette.make(for: colorScheme) }

    private var currentDirections: [String] {
        wave.direction ?? []
    }

    private var availableDirections: [Direction] {
        repoState.directions.filter { dir in
            !currentDirections.contains(dir.name)
        }
    }

    var body: some View {
        HStack(spacing: Spacing.sm) {
            Label("Direction", systemImage: "target")
                .font(.caption)
                .foregroundStyle(.secondary)

            // Current direction pills
            ForEach(currentDirections, id: \.self) { direction in
                DirectionPill(
                    name: direction,
                    onRemove: { removeDirection(direction) }
                )
            }

            // Add button
            if !availableDirections.isEmpty {
                Button {
                    showingPicker = true
                } label: {
                    Image(systemName: "plus")
                        .font(.system(size: 10, weight: .medium))
                        .foregroundStyle(.secondary)
                        .padding(6)
                        .background(Circle().fill(palette.surface))
                }
                .buttonStyle(.plain)
                .popover(isPresented: $showingPicker) {
                    directionPicker
                }
            }

            if isSaving {
                ProgressView()
                    .scaleEffect(0.6)
            }
        }
    }

    private var directionPicker: some View {
        VStack(alignment: .leading, spacing: Spacing.sm) {
            Text("Add Direction")
                .font(.headline)
                .padding(.bottom, Spacing.xs)

            ForEach(availableDirections) { direction in
                Button {
                    addDirection(direction.name)
                    showingPicker = false
                } label: {
                    HStack {
                        Text(direction.displayName)
                            .font(.subheadline)
                        Spacer()
                    }
                    .padding(.vertical, Spacing.xs)
                    .contentShape(Rectangle())
                }
                .buttonStyle(.plain)
            }
        }
        .padding(Spacing.md)
        .frame(minWidth: 180)
    }

    private func addDirection(_ name: String) {
        var newDirections = currentDirections
        newDirections.append(name)
        updateDirections(newDirections)
    }

    private func removeDirection(_ name: String) {
        var newDirections = currentDirections
        newDirections.removeAll { $0 == name }
        updateDirections(newDirections)
    }

    private func updateDirections(_ directions: [String]) {
        isSaving = true
        Task {
            do {
                try await repoState.updateWave(wave, direction: directions)
            } catch {
                // Silent failure - UI will show previous state
            }
            await MainActor.run {
                isSaving = false
            }
        }
    }
}

struct DirectionPill: View {
    let name: String
    var onRemove: () -> Void

    @Environment(\.colorScheme) private var colorScheme
    private var palette: LoopflowPalette { LoopflowPalette.make(for: colorScheme) }

    var body: some View {
        HStack(spacing: Spacing.xs) {
            Text(displayName)
                .font(.system(size: 11, weight: .medium))

            Button {
                onRemove()
            } label: {
                Image(systemName: "xmark")
                    .font(.system(size: 8, weight: .semibold))
            }
            .buttonStyle(.plain)
            .opacity(0.7)
        }
        .padding(.horizontal, Spacing.sm)
        .padding(.vertical, Spacing.xs)
        .background(Capsule().fill(palette.accent.opacity(0.15)))
        .foregroundStyle(palette.accent)
    }

    private var displayName: String {
        name.replacingOccurrences(of: "-", with: " ").capitalized
    }
}

#Preview {
    let repoState = RepoState()
    repoState.configureMockWaves()
    repoState.directions = [
        Direction(id: "product-engineer", name: "product-engineer", content: "", path: URL(fileURLWithPath: "/")),
        Direction(id: "designer", name: "designer", content: "", path: URL(fileURLWithPath: "/")),
        Direction(id: "security", name: "security", content: "", path: URL(fileURLWithPath: "/"))
    ]

    let wave = Wave(
        id: "test",
        name: "test-wave",
        area: ["."],
        direction: ["product-engineer"],
        flow: "design",
        repo: "/tmp/test-repo"
    )

    return DirectionPills(wave: wave)
        .environment(repoState)
        .padding()
}
