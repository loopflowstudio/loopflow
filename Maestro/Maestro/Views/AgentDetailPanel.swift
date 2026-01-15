// Detail panel for viewing/editing an agent's configuration.

import SwiftUI

struct AgentDetailPanel: View {
    let agent: Agent
    let onSave: (Agent) -> Void
    let onStart: () -> Void
    let onStop: () -> Void
    let onDelete: () -> Void

    // Editable state
    @State private var goal: String = ""
    @State private var pipeline: String = ""
    @State private var triggerKind: TriggerKind = .manual
    @State private var cronExpression: String = ""
    @State private var contextPaths: [String] = []
    @State private var mergeStrategy: MergeStrategy = .pr

    @State private var hasChanges = false
    @State private var showingDeleteConfirm = false
    @State private var showingContextPicker = false
    @State private var newContextPath = ""

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 24) {
                headerSection
                configSection
                Divider()
                statusSection
            }
            .padding(24)
        }
        .onAppear {
            loadFromAgent()
        }
        .onChange(of: agent.id) { _, _ in
            loadFromAgent()
        }
        .toolbar {
            ToolbarItem(placement: .primaryAction) {
                if hasChanges {
                    Button("Save") {
                        saveChanges()
                    }
                    .buttonStyle(.borderedProminent)
                }
            }
        }
        .confirmationDialog("Delete Agent?", isPresented: $showingDeleteConfirm) {
            Button("Delete", role: .destructive) {
                onDelete()
            }
            Button("Cancel", role: .cancel) {}
        } message: {
            Text("This will delete the agent definition file. This cannot be undone.")
        }
    }

    // MARK: - Header

    private var headerSection: some View {
        HStack(spacing: 12) {
            Text(agent.displayEmoji)
                .font(.system(size: 48))

            VStack(alignment: .leading, spacing: 4) {
                Text(agent.name)
                    .font(.title2)
                    .fontWeight(.semibold)

                Text(agent.repo)
                    .font(.caption)
                    .foregroundStyle(.secondary)
            }

            Spacer()

            Button(role: .destructive) {
                showingDeleteConfirm = true
            } label: {
                Image(systemName: "trash")
            }
            .buttonStyle(.borderless)
        }
    }

    // MARK: - Config Section

    private var configSection: some View {
        VStack(alignment: .leading, spacing: 16) {
            // Goal
            VStack(alignment: .leading, spacing: 6) {
                Text("Goal")
                    .font(.caption)
                    .foregroundStyle(.secondary)

                TextEditor(text: $goal)
                    .font(.body)
                    .scrollContentBackground(.hidden)
                    .padding(8)
                    .frame(minHeight: 100, maxHeight: 200)
                    .background(
                        RoundedRectangle(cornerRadius: 8)
                            .fill(Color.primary.opacity(0.05))
                    )
                    .onChange(of: goal) { _, _ in hasChanges = true }
            }

            // Pipeline
            HStack {
                Text("Pipeline")
                    .font(.caption)
                    .foregroundStyle(.secondary)
                    .frame(width: 80, alignment: .leading)

                TextField("ship", text: $pipeline)
                    .textFieldStyle(.roundedBorder)
                    .onChange(of: pipeline) { _, _ in hasChanges = true }
            }

            // Trigger
            VStack(alignment: .leading, spacing: 8) {
                HStack {
                    Text("Trigger")
                        .font(.caption)
                        .foregroundStyle(.secondary)
                        .frame(width: 80, alignment: .leading)

                    Picker("", selection: $triggerKind) {
                        ForEach(TriggerKind.allCases, id: \.self) { kind in
                            Text(kind.displayName).tag(kind)
                        }
                    }
                    .pickerStyle(.segmented)
                    .onChange(of: triggerKind) { _, _ in hasChanges = true }
                }

                if triggerKind == .cron {
                    HStack {
                        Spacer()
                            .frame(width: 80)

                        TextField("0 9 * * *", text: $cronExpression)
                            .textFieldStyle(.roundedBorder)
                            .frame(width: 150)
                            .onChange(of: cronExpression) { _, _ in hasChanges = true }

                        Text("Cron expression")
                            .font(.caption)
                            .foregroundStyle(.tertiary)
                    }
                }
            }

            // Merge Strategy
            HStack {
                Text("Merge")
                    .font(.caption)
                    .foregroundStyle(.secondary)
                    .frame(width: 80, alignment: .leading)

                Picker("", selection: $mergeStrategy) {
                    ForEach(MergeStrategy.allCases, id: \.self) { strategy in
                        Text(strategy.displayName).tag(strategy)
                    }
                }
                .pickerStyle(.segmented)
                .frame(width: 200)
                .onChange(of: mergeStrategy) { _, _ in hasChanges = true }
            }

            // Context
            VStack(alignment: .leading, spacing: 6) {
                HStack {
                    Text("Context")
                        .font(.caption)
                        .foregroundStyle(.secondary)

                    Spacer()

                    Button {
                        showingContextPicker = true
                    } label: {
                        Image(systemName: "plus")
                            .font(.caption)
                    }
                    .buttonStyle(.borderless)
                    .popover(isPresented: $showingContextPicker) {
                        contextPickerPopover()
                    }
                }

                if contextPaths.isEmpty {
                    Text("No context files configured")
                        .font(.caption)
                        .foregroundStyle(.tertiary)
                        .italic()
                } else {
                    HStack {
                        ForEach(contextPaths, id: \.self) { path in
                            HStack(spacing: 4) {
                                Text(path)
                                    .font(.caption)
                                Button {
                                    contextPaths.removeAll { $0 == path }
                                    hasChanges = true
                                } label: {
                                    Image(systemName: "xmark")
                                        .font(.caption2)
                                }
                                .buttonStyle(.plain)
                            }
                            .padding(.horizontal, 8)
                            .padding(.vertical, 4)
                            .background(Capsule().fill(Color.orange.opacity(0.15)))
                        }
                    }
                }
            }
        }
    }

    // MARK: - Status Section

    private var statusSection: some View {
        VStack(alignment: .leading, spacing: 12) {
            Text("Status")
                .font(.caption)
                .foregroundStyle(.secondary)

            HStack(spacing: 8) {
                statusDot(agent.status)
                Text(statusText)
                    .fontWeight(.medium)
            }

            if let worktree = agent.worktree, agent.status == .running {
                HStack {
                    Text("Worktree")
                        .font(.caption)
                        .foregroundStyle(.secondary)
                    Text(worktree)
                        .font(.caption.monospaced())
                }
            }

            if let lastRun = agent.lastRunText {
                HStack {
                    Text("Last run")
                        .font(.caption)
                        .foregroundStyle(.secondary)
                    Text(lastRun)
                        .font(.caption)
                }
            }

            if agent.iteration > 0 {
                HStack {
                    Text("Iterations")
                        .font(.caption)
                        .foregroundStyle(.secondary)
                    Text("\(agent.iteration)")
                        .font(.caption)
                }
            }

            // Actions
            HStack(spacing: 12) {
                switch agent.status {
                case .running, .waiting:
                    Button("Stop") {
                        onStop()
                    }
                    .buttonStyle(.bordered)

                    if agent.worktree != nil {
                        Button("Open Worktree") {
                            openWorktree()
                        }
                    }

                case .idle, .completed, .stopped, .error:
                    Button("Start") {
                        onStart()
                    }
                    .buttonStyle(.borderedProminent)
                }
            }
            .padding(.top, 8)
        }
    }

    private var statusText: String {
        switch agent.status {
        case .running:
            return "Running (iteration \(agent.iteration))"
        case .waiting:
            return "Waiting"
        case .idle:
            return "Idle"
        case .completed:
            return "Completed (iteration \(agent.iteration))"
        case .error:
            return "Error"
        case .stopped:
            return "Stopped"
        }
    }

    private func statusDot(_ status: AgentStatus) -> some View {
        Circle()
            .fill(status.color)
            .frame(width: 8, height: 8)
    }

    // MARK: - Context Picker

    private func contextPickerPopover() -> some View {
        VStack(alignment: .leading, spacing: 12) {
            Text("Add Context Path")
                .font(.caption)
                .foregroundStyle(.secondary)

            HStack {
                TextField("src/schema.py", text: $newContextPath)
                    .textFieldStyle(.roundedBorder)
                    .frame(width: 200)

                Button("Add") {
                    if !newContextPath.isEmpty {
                        contextPaths.append(newContextPath)
                        hasChanges = true
                        newContextPath = ""
                        showingContextPicker = false
                    }
                }
                .disabled(newContextPath.isEmpty)
            }

            Text("Relative to repo root")
                .font(.caption2)
                .foregroundStyle(.tertiary)
        }
        .padding(12)
    }

    // MARK: - Actions

    private func loadFromAgent() {
        goal = agent.prompt
        pipeline = agent.pipeline
        triggerKind = agent.triggerKind
        cronExpression = agent.cron ?? ""
        contextPaths = agent.context
        mergeStrategy = agent.mergeStrategy
        hasChanges = false
    }

    private func saveChanges() {
        let updated = Agent(
            id: agent.id,
            name: agent.name,
            repo: agent.repo,
            pipeline: pipeline,
            triggerKind: triggerKind,
            context: contextPaths,
            prompt: goal,
            emoji: agent.emoji,
            cron: triggerKind == .cron ? cronExpression : nil,
            mergeStrategy: mergeStrategy,
            status: agent.status,
            lastRunAt: agent.lastRunAt,
            iteration: agent.iteration,
            pid: agent.pid,
            worktree: agent.worktree
        )
        onSave(updated)
        hasChanges = false
    }

    private func openWorktree() {
        guard let worktree = agent.worktree else { return }
        let url = URL(fileURLWithPath: worktree)
        NSWorkspace.shared.open(url)
    }
}
