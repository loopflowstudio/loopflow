// Sidebar row for waves discovered from the configured Asana team but not
// actively managed by loopflow yet. Shows the wave name and a "Start"
// affordance that creates the lfd store record on demand.

import LoopflowCore
import SwiftUI

struct UnmanagedWaveRow: View {
  let wave: DiscoveredWaveSummary
  @Environment(RepoState.self) private var repoState

  @State private var isHovering = false
  @State private var isStarting = false
  @State private var startError: String?

  var body: some View {
    VStack(alignment: .leading, spacing: 4) {
      HStack(spacing: 8) {
        Image(systemName: "circle.dashed")
          .font(Typography.caption(11))
          .foregroundStyle(.white.opacity(0.4))

        Text(wave.waveName)
          .fontWeight(.medium)
          .foregroundStyle(.white.opacity(0.85))
          .lineLimit(1)

        Spacer()

        if isHovering || isStarting {
          Button {
            startWorking()
          } label: {
            if isStarting {
              ProgressView()
                .controlSize(.mini)
                .tint(.white)
            } else {
              Text("Start")
                .font(Typography.caption(10))
                .fontWeight(.semibold)
                .padding(.horizontal, 8)
                .padding(.vertical, 2)
                .background(Color.white.opacity(0.18))
                .foregroundStyle(.white)
                .clipShape(Capsule())
            }
          }
          .buttonStyle(.plain)
          .disabled(isStarting)
          .help("Create a managed wave for this Asana project")
        }
      }

      if wave.asanaProjectId == nil {
        Text("Not yet linked to Asana")
          .font(Typography.caption(10))
          .foregroundStyle(.white.opacity(0.4))
      }

      if let startError {
        Text(startError)
          .font(Typography.caption(10))
          .foregroundStyle(Color.statusError)
          .lineLimit(2)
      }
    }
    .padding(.horizontal, Spacing.md)
    .padding(.vertical, Spacing.xs)
    .background(
      RoundedRectangle(cornerRadius: CornerRadius.md)
        .fill(isHovering ? Color.white.opacity(0.06) : .clear)
    )
    .contentShape(RoundedRectangle(cornerRadius: CornerRadius.md))
    .hoverTracking { isHovering = $0 }
  }

  private func startWorking() {
    guard !isStarting else { return }
    isStarting = true
    startError = nil
    Task { @MainActor in
      defer { isStarting = false }
      do {
        let created = try await repoState.createWave(name: wave.waveName)
        await repoState.refreshDiscoveredWaves()
        repoState.selectedWaveId = created.id
      } catch {
        startError = error.localizedDescription
      }
    }
  }
}
