#if os(macOS)
import Loopflow
import SwiftUI

struct ChapterHistoryView: View {
    let wave: String
    let repo: String
    var sourceReference: String? = nil
    @Environment(\.dismiss) private var dismiss
    @State private var history: [ChapterHistoryEntry] = []
    @State private var selected: String?
    @State private var snapshot: ChapterSnapshot?
    @State private var error: String?

    var body: some View {
        VStack(alignment: .leading, spacing: 16) {
            HStack {
                Text("\(wave) · Chapter history").font(.headline)
                Spacer()
                Button("Done") { dismiss() }
            }
            if let error { Text(error).foregroundStyle(Color.statusWarning) }
            if history.isEmpty, error == nil { ProgressView("Reading chapters…") }
            if !history.isEmpty {
                Picker("Chapter", selection: $selected) {
                    ForEach(history) { chapter in
                        Text(chapter.id + (chapter.phase == "preparing" ? " · preparing" : chapter.closedAt == nil ? " · current" : ""))
                            .tag(Optional(chapter.id))
                    }
                }
            }
            ScrollView {
                if let snapshot {
                    VStack(alignment: .leading, spacing: 12) {
                        Text("Evidence as of \(Date(timeIntervalSince1970: TimeInterval(snapshot.observedAt)).formatted())")
                            .font(.caption).foregroundStyle(.secondary)
                        Text("Metrics evaluated at \(Date(timeIntervalSince1970: TimeInterval(snapshot.metricsEvaluatedAt)).formatted())")
                            .font(.caption).foregroundStyle(.secondary)
                        WaveMetricPortfolioView(portfolio: snapshot.metrics)
                        ForEach(snapshot.content.krs) { kr in
                            Label(kr.text, systemImage: kr.holds ? "checkmark.circle.fill" : "circle")
                        }
                        ForEach(snapshot.tasks) { item in
                            VStack(alignment: .leading, spacing: 4) {
                                Text("\(item.task.identifier) · \(item.task.name)")
                                Text("\(item.disposition.capitalized) · \(item.reason)")
                                    .font(.caption).foregroundStyle(.secondary)
                            }
                        }
                    }.frame(maxWidth: .infinity, alignment: .leading)
                }
            }
        }
        .padding(24).frame(minWidth: 500, minHeight: 420)
        .task {
            do {
                history = try await RegistryQueryLocal.shared.chapterHistory(wave: wave, cwd: repo)
                selected = history.first(where: { $0.sourceProjectId == sourceReference || $0.sourceProjectSlug == sourceReference || (sourceReference != nil && $0.sourceWorkId == sourceReference) })?.id
                    ?? history.first(where: { $0.closedAt == nil && $0.phase != "preparing" })?.id ?? history.last?.id
                if history.isEmpty { error = "No chapters recorded for this Wave." }
            } catch { self.error = error.localizedDescription }
        }
        .task(id: selected) {
            snapshot = nil
            guard let selected else { return }
            do {
                snapshot = try await RegistryQueryLocal.shared.chapter(wave: wave, id: selected, cwd: repo)
                error = nil
            } catch { self.error = error.localizedDescription }
        }
    }
}
#endif
