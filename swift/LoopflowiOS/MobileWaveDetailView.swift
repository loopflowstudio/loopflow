#if os(iOS)
import SwiftUI
import Loopflow

struct MobileWaveDetailView: View {
    let waveId: String

    @Environment(RepoState.self) private var repoState
    @Environment(OutputBuffer.self) private var outputBuffer
    @Environment(\.palette) private var palette
    @Environment(\.scenePhase) private var scenePhase

    private var wave: WaveViewModel? {
        repoState.waveStore.wave(for: waveId)
    }

    var body: some View {
        Group {
            if let wave {
                detailContent(for: wave)
            } else {
                ContentUnavailableView("Select a Wave", systemImage: "waveform.path.ecg")
            }
        }
        .background(palette.background)
    }

    private func detailContent(for wave: WaveViewModel) -> some View {
        VStack(spacing: Spacing.md) {
            VStack(alignment: .leading, spacing: Spacing.xs) {
                HStack(alignment: .firstTextBaseline) {
                    Text(wave.displayName)
                        .font(Typography.sectionTitle())
                    Spacer()
                    Text(wave.statusText)
                        .font(Typography.caption())
                        .foregroundStyle(wave.status.color)
                }
                Text(wave.areaDisplay)
                    .font(Typography.caption())
                    .foregroundStyle(palette.textSecondary)
            }
            .frame(maxWidth: .infinity, alignment: .leading)
            .padding(.horizontal, Spacing.lg)

            LiveOutput(lines: outputBuffer.output(for: wave.id))
                .padding(.horizontal, Spacing.lg)

            Spacer(minLength: 0)
        }
        .navigationTitle(wave.displayName)
        .navigationBarTitleDisplayMode(.inline)
        .task {
            outputBuffer.startStreaming(waveId: wave.id)
            repoState.loadWaveContent(for: wave.id)
        }
        .onChange(of: scenePhase) { _, phase in
            guard phase == .active else { return }
            outputBuffer.stopStreaming(waveId: wave.id)
            outputBuffer.startStreaming(waveId: wave.id)
        }
        .onDisappear {
            outputBuffer.stopStreaming(waveId: wave.id)
        }
    }
}

#endif
