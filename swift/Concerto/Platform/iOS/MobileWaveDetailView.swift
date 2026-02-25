#if os(iOS)
import SwiftUI
import LoopflowCore

struct MobileWaveDetailView: View {
    private enum Tab: String, CaseIterable {
        case output = "Output"
        case chat = "Chat"
    }

    let waveId: String

    @Environment(RepoState.self) private var repoState
    @Environment(OutputBuffer.self) private var outputBuffer
    @Environment(\.palette) private var palette
    @Environment(\.openURL) private var openURL

    @State private var tab: Tab = .output

    private var wave: WaveViewModel? {
        repoState.waveStore.wave(for: waveId)
    }

    var body: some View {
        Group {
            if let wave {
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

                        if let prURL = wave.prURL {
                            Button {
                                openURL(prURL)
                            } label: {
                                Label("Open PR", systemImage: "arrow.up.right.square")
                            }
                            .buttonStyle(.bordered)
                        }
                    }
                    .frame(maxWidth: .infinity, alignment: .leading)
                    .padding(.horizontal, Spacing.lg)

                    Picker("", selection: $tab) {
                        Text(Tab.output.rawValue).tag(Tab.output)
                        Text(Tab.chat.rawValue).tag(Tab.chat)
                    }
                    .pickerStyle(.segmented)
                    .padding(.horizontal, Spacing.lg)

                    if tab == .output {
                        LiveOutput(lines: outputBuffer.output(for: wave.id))
                            .padding(.horizontal, Spacing.lg)
                    } else {
                        WaveChatView(state: repoState.chatState(for: wave.id))
                    }

                    Spacer(minLength: 0)
                }
                .navigationTitle(wave.displayName)
                .navigationBarTitleDisplayMode(.inline)
                .task {
                    outputBuffer.startStreaming(waveId: wave.id)
                    repoState.loadRuns(for: wave.id)
                    repoState.loadWaveContent(for: wave.id)
                }
                .onDisappear {
                    outputBuffer.stopStreaming(waveId: wave.id)
                }
            } else {
                ContentUnavailableView("Select a Wave", systemImage: "waveform.path.ecg")
            }
        }
        .background(palette.background)
    }
}

#endif
