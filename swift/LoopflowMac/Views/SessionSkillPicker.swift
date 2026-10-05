import Loopflow
import SwiftUI

struct SessionSkillPicker: View {
    let model: PodiumModel
    let repo: String
    let dismiss: () -> Void
    @Environment(\.palette) private var palette
    @FocusState private var searching: Bool
    @State private var skills: PodiumReading<[DiscoveryEntry]> = .loading
    @State private var search = ""
    @State private var active: String?

    private var matches: [DiscoveryEntry] {
        (skills.value ?? []).filter {
            search.isEmpty || $0.name.localizedCaseInsensitiveContains(search)
                || $0.description.localizedCaseInsensitiveContains(search)
        }
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            TextField("Search skills…", text: $search)
                .textFieldStyle(.plain)
                .focused($searching)
                .accessibilityIdentifier("session-skill-search")
                .padding(8)
                .onSubmit { chooseActive() }
                .onKeyPress(.downArrow) { move(1); return .handled }
                .onKeyPress(.upArrow) { move(-1); return .handled }
            Divider()
            if skills.isLoading {
                ProgressView().frame(maxWidth: .infinity)
            } else if let error = skills.errorMessage {
                Text(error).foregroundStyle(palette.textSecondary)
                Button("Retry") { Task { await load() } }
            } else if matches.isEmpty {
                Text("No matching skills").foregroundStyle(palette.textSecondary).padding(8)
            } else {
                ScrollViewReader { scroll in
                    ScrollView {
                        LazyVStack(alignment: .leading, spacing: 2) {
                            ForEach(matches) { skill in
                                Button { choose(skill.name) } label: {
                                    HStack(alignment: .top, spacing: 9) {
                                        Image(systemName: "checkmark")
                                            .opacity(skill.name == model.selectedSessionSkill ? 1 : 0)
                                            .frame(width: 14)
                                        VStack(alignment: .leading, spacing: 3) {
                                            Text(skill.name).font(.system(size: 13, weight: .medium))
                                            Text(skill.description).font(.system(size: 11))
                                                .foregroundStyle(palette.textSecondary)
                                        }
                                        Spacer(minLength: 0)
                                    }
                                    .padding(9)
                                    .frame(maxWidth: .infinity, alignment: .leading)
                                    .background(active == skill.name ? palette.surfaceMuted : Color.clear,
                                                in: RoundedRectangle(cornerRadius: 4))
                                    .contentShape(Rectangle())
                                }
                                .buttonStyle(.plain)
                                .id(skill.name)
                                .accessibilityIdentifier("session-skill-\(skill.name)")
                            }
                        }
                    }
                    .onAppear {
                        if let active { scroll.scrollTo(active) }
                    }
                    .onChange(of: active) { _, name in
                        if let name { scroll.scrollTo(name) }
                    }
                }
                .frame(maxHeight: 320)
            }
            Divider()
            Text("↑ ↓ move   ↵ select   esc close")
                .font(.system(size: 10)).foregroundStyle(palette.textSecondary).padding(4)
        }
        .padding(8)
        .frame(width: 350)
        .font(.system(size: 13))
        .foregroundStyle(palette.text)
        .background(palette.surface)
        .onExitCommand(perform: dismiss)
        .onChange(of: search) { _, _ in active = matches.first?.name }
        .task { searching = true; await load() }
    }

    private func load() async {
        skills = .loading
        do {
            let result = try await model.sessionSkills(repo: repo)
            guard !Task.isCancelled else { return }
            skills = .available(result)
            active = matches.first(where: { $0.name == model.selectedSessionSkill })?.name ?? matches.first?.name
        } catch {
            guard !Task.isCancelled else { return }
            skills = .unavailable(lastGood: nil, reason: error.localizedDescription)
        }
    }

    private func move(_ offset: Int) {
        let rows = matches
        guard !rows.isEmpty else { return }
        let index = rows.firstIndex(where: { $0.name == active }) ?? 0
        active = rows[(index + offset + rows.count) % rows.count].name
    }

    private func chooseActive() {
        guard let active, matches.contains(where: { $0.name == active }) else { return }
        choose(active)
    }

    private func choose(_ name: String) {
        model.selectSessionSkill(name, repo: repo)
        dismiss()
    }
}
