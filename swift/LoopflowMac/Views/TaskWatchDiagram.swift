#if os(macOS)
import Loopflow
import SwiftUI

/// Geometry is presentation only; every node and transition comes from the snapshot.
struct TaskWatchDiagram: View {
    @Bindable var store: TaskWatchStore
    @Environment(\.palette) private var palette
    @FocusState private var focusedStage: UInt32?

    var body: some View {
        if let invocation = store.invocation, !invocation.stages.isEmpty {
            VStack(alignment: .leading, spacing: 0) {
                Text("Dashed: plan · Solid: recorded transitions")
                    .font(Typography.caption(10))
                    .foregroundStyle(palette.textSecondary)
                    .padding(Spacing.sm)
                ScrollViewReader { scroll in
                    ScrollView {
                        VStack(alignment: .leading, spacing: 16) {
                            ForEach(Array(invocation.stages.enumerated()), id: \.element.stepIndex) { index, stage in
                                VStack(alignment: .leading, spacing: Spacing.xs) {
                                    if index == 0 || stage.flowParents != invocation.stages[index - 1].flowParents {
                                        Text(stage.flowParents.joined(separator: " › "))
                                            .font(Typography.caption(10).weight(.semibold))
                                            .foregroundStyle(palette.textSecondary)
                                            .background(palette.background)
                                    }
                                    node(stage, invocation: invocation)
                                    ForEach(Array(invocation.transitions.enumerated()), id: \.offset) { _, edge in
                                        if edge.from.invocationId == invocation.id,
                                           edge.from.stepIndex == stage.stepIndex,
                                           edge.reason == .iterated || edge.reason == .retried {
                                            Button {
                                                store.selectStage(edge.to)
                                            } label: {
                                                Label(
                                                    "\(edge.reason.rawValue.capitalized) → stage \(edge.to.stepIndex + 1) · iteration \(edge.to.iteration)",
                                                    systemImage: "arrow.uturn.backward"
                                                )
                                            }
                                            .font(Typography.caption(10))
                                            .buttonStyle(.link)
                                            .background(palette.background)
                                            .accessibilityLabel("\(edge.reason.rawValue): stage \(edge.from.stepIndex + 1), iteration \(edge.from.iteration) to stage \(edge.to.stepIndex + 1), iteration \(edge.to.iteration)")
                                        }
                                    }
                                }
                                .id(stage.stepIndex)
                            }
                        }
                        .padding(.leading, 44)
                        .padding(.trailing, Spacing.sm)
                        .padding(.vertical, Spacing.sm)
                        .backgroundPreferenceValue(StageAnchors.self) { anchors in
                            GeometryReader { geometry in
                                connections(invocation, frames: anchors.mapValues { geometry[$0] })
                            }
                            .allowsHitTesting(false)
                            .accessibilityHidden(true)
                        }
                    }
                    .onChange(of: store.stepIndex) { _, selected in
                        if let selected { scroll.scrollTo(selected) }
                    }
                    .onAppear {
                        if let selected = store.stepIndex { scroll.scrollTo(selected) }
                    }
                }
            }
            .id(invocation.id)
            .foregroundStyle(palette.text)
            .background(palette.background)
            .accessibilityLabel("Recorded flow diagram")
        } else {
            ContentUnavailableView(
                "No retained stages", systemImage: "point.3.connected.trianglepath.dotted",
                description: Text("This invocation has no retained expanded plan.")
            )
        }
    }

    private func node(_ stage: TaskWatchStage, invocation: TaskWatchInvocation) -> some View {
        let active = store.snapshot?.activeStage?.invocationId == invocation.id
            && store.snapshot?.activeStage?.stepIndex == stage.stepIndex
        let selected = store.stepIndex == stage.stepIndex
        return Button {
            store.inspectStage(stage.stepIndex)
        } label: {
            VStack(alignment: .leading, spacing: Spacing.xs) {
                HStack {
                    Label("\(stage.stepIndex + 1). \(stage.name)", systemImage: stage.human
                        ? "person.crop.circle" : stage.kind == .op ? "terminal" : "square.stack.3d.up")
                        .font(Typography.body(12).weight(.semibold))
                    if active {
                        Image(systemName: "location.fill")
                            .help("Current stage")
                            .accessibilityLabel("Current stage")
                    }
                }
                Text("\(stage.human ? "Human checkpoint" : stage.kind.rawValue) · \(stage.attempts.last?.state.rawValue ?? "Not entered") · \(stage.attempts.count) \(stage.attempts.count == 1 ? "attempt" : "attempts")")
                    .font(Typography.caption(10))
                    .foregroundStyle(palette.textSecondary)
            }
            .frame(maxWidth: .infinity, alignment: .leading)
            .padding(Spacing.sm)
            .background(selected ? palette.surfaceMuted : palette.surface)
            .clipShape(RoundedRectangle(cornerRadius: CornerRadius.md))
            .overlay {
                RoundedRectangle(cornerRadius: CornerRadius.md)
                    .strokeBorder(selected || active ? palette.accent : palette.border,
                                  lineWidth: selected ? 2 : 1)
            }
        }
        .buttonStyle(.plain)
        .focused($focusedStage, equals: stage.stepIndex)
        .onMoveCommand { direction in
            guard direction == .up || direction == .down,
                  let index = invocation.stages.firstIndex(where: { $0.stepIndex == stage.stepIndex }) else { return }
            let next = direction == .up ? index - 1 : index + 1
            guard invocation.stages.indices.contains(next) else { return }
            let target = invocation.stages[next].stepIndex
            focusedStage = target
            store.inspectStage(target)
        }
        .accessibilityIdentifier("watch-stage-\(invocation.id)-\(stage.stepIndex)")
        .accessibilityAddTraits(selected ? [.isSelected] : [])
        .anchorPreference(key: StageAnchors.self, value: .bounds) { [stage.stepIndex: $0] }
    }

    private func connections(_ invocation: TaskWatchInvocation, frames: [UInt32: CGRect]) -> some View {
        Canvas { context, _ in
            for (from, to) in zip(invocation.stages, invocation.stages.dropFirst()) {
                guard let start = frames[from.stepIndex], let end = frames[to.stepIndex] else { continue }
                var path = Path()
                path.move(to: CGPoint(x: start.midX, y: start.maxY))
                path.addLine(to: CGPoint(x: end.midX, y: end.minY))
                context.stroke(path, with: .color(palette.textSecondary.opacity(0.5)),
                               style: StrokeStyle(lineWidth: 1, dash: [3, 4]))
            }
            for (index, edge) in invocation.transitions.enumerated() {
                guard edge.from.invocationId == invocation.id, edge.to.invocationId == invocation.id,
                      let start = frames[edge.from.stepIndex], let end = frames[edge.to.stepIndex] else { continue }
                // Separate side lanes keep return and same-stage retry arrows off the nodes.
                let lane = CGFloat(10 + (index % 3) * 10)
                let from = CGPoint(x: start.minX, y: start.midY + 9)
                let to = CGPoint(x: end.minX, y: end.midY - 9)
                var path = Path()
                path.move(to: from)
                path.addLine(to: CGPoint(x: lane, y: from.y))
                path.addLine(to: CGPoint(x: lane, y: to.y))
                path.addLine(to: to)
                path.move(to: CGPoint(x: to.x - 5, y: to.y - 4))
                path.addLine(to: to)
                path.addLine(to: CGPoint(x: to.x - 5, y: to.y + 4))
                context.stroke(path, with: .color(palette.accent), style: StrokeStyle(lineWidth: 1.5, lineJoin: .round))
            }
        }
    }
}

private struct StageAnchors: PreferenceKey {
    static let defaultValue: [UInt32: Anchor<CGRect>] = [:]
    static func reduce(value: inout [UInt32: Anchor<CGRect>], nextValue: () -> [UInt32: Anchor<CGRect>]) {
        value.merge(nextValue(), uniquingKeysWith: { _, new in new })
    }
}
#endif
