#if os(iOS)
import SwiftUI
import LoopflowCore

struct MobileWaveListView: View {
    @Environment(RepoState.self) private var repoState
    @Environment(OutputBuffer.self) private var outputBuffer
    @Environment(\.palette) private var palette

    @Binding var selectedWaveId: String?

    private var waves: [WaveViewModel] { repoState.waves }

    var body: some View {
        if waves.isEmpty {
            ContentUnavailableView("No Waves", systemImage: "waveform.path")
        } else {
            List(waves, selection: $selectedWaveId) { wave in
                NavigationLink(value: wave.id) {
                    VStack(alignment: .leading, spacing: Spacing.xxs) {
                        HStack {
                            Text(wave.displayName)
                                .font(Typography.body())
                            Spacer()
                            Text(wave.statusText)
                                .font(Typography.caption(11))
                                .foregroundStyle(wave.status.color)
                        }
                        Text(wave.areaDisplay)
                            .font(Typography.caption(11))
                            .foregroundStyle(palette.textSecondary)
                            .lineLimit(1)
                        if let recentOutput = outputBuffer.recentOutput(for: wave.id, maxLength: 80) {
                            Text(recentOutput)
                                .font(Typography.code(11))
                                .foregroundStyle(palette.textSecondary)
                                .lineLimit(1)
                        }
                    }
                }
                .tag(wave.id)
            }
            .listStyle(.plain)
        }
        .navigationTitle("Waves")
        .refreshable {
            await repoState.refreshWaves()
        }
    }
}

#endif
