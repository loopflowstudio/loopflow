import Testing
@testable import Loopflow

// The information hierarchy, pinned against a realistic long-running `task`
// flow: clarify reads the repo and writes a design note, pursue builds and hits
// a red test, mutate opens the PR, and a child project asks for a decision.
//
// What a human must see: the wave's prose, the red test, the decision. What
// must collapse: twenty-odd tool calls, shell commands, file edits, thoughts,
// and the `task_clarify` / `task_pursue` / `task_mutate` phase structure.

private func turn(
    id: String,
    role: ChatRole = .assistant,
    text: String = "",
    status: Lifecycle = .completed,
    items: [ConversationItem] = [],
    from: String? = nil,
    body: BodyProvenance? = nil,
    activity: ChildControlActivity? = nil
) throws -> ChatTurn {
    try ChatTurn(
        id: id,
        role: role,
        text: text,
        status: status,
        items: items,
        createdAt: "2026-07-14T09:00:00Z",
        from: from,
        body: body,
        activity: activity
    )
}

private func command(_ id: String, _ argv: String, exit: Int = 0, status: Lifecycle = .completed) -> ConversationItem {
    .command(
        id: id,
        command: argv.split(separator: " ").map(String.init),
        cwd: "/repo",
        status: status,
        output: "…",
        exitCode: exit,
        durationMs: 900
    )
}

private func tool(_ id: String, _ name: String, status: Lifecycle = .completed) -> ConversationItem {
    .tool(id: id, name: name, status: status, input: .string("…"), output: "ok")
}

private func edit(_ id: String, _ path: String) -> ConversationItem {
    .file(id: id, changes: [FileEdit(path: path, kind: "modified", diff: nil)], status: .completed)
}

private func provenance(step: String) -> BodyProvenance {
    BodyProvenance(
        bodyId: "body-\(step)",
        invocationId: "inv-1",
        stepIndex: 0,
        flow: "task",
        step: step,
        iteration: 0,
        sessionId: "sess-1",
        harness: "claude",
        model: "opus",
        host: "mini",
        worktree: "/repo",
        startedAt: "2026-07-14T09:00:00Z",
        endedAt: nil,
        terminationReason: nil
    )
}

private func childActivity(_ kind: ChildActivityKind) -> ChildControlActivity {
    ChildControlActivity(
        id: "act-\(kind.rawValue)",
        subject: .project,
        subjectId: "wave-chat",
        sessionId: "ps_1",
        kind: kind,
        title: "Wave chat",
        summary: "…",
        directiveVersion: nil,
        commandId: nil,
        effect: nil,
        source: .wave(id: "w1"),
        decisionId: nil,
        options: []
    )
}

@Suite("Wave chat reads as a conversation")
struct WaveChatTranscriptTests {
    /// task_clarify: five items of machinery behind one sentence of speech.
    @Test func machineryCollapsesToOneActivityRow() throws {
        let clarify = try turn(
            id: "turn-2",
            text: "The transcript renders every tool call as a card. I'll collapse them behind an audit toggle.",
            items: [
                .thought(id: "h0", text: "weighing the options"),
                tool("t0", "Read"),
                tool("t1", "Grep"),
                command("c0", "git log --oneline -3"),
                edit("f0", "scratch/w2-129.md"),
            ],
            body: provenance(step: "task_clarify")
        )

        let view = turnPresentation(clarify, audit: false)
        #expect(view.prose.hasPrefix("The transcript renders"))
        #expect(view.activity?.label == "1 command · 1 file edited · 2 tool calls")
        #expect(view.failures.isEmpty)
        #expect(view.auditItems.isEmpty, "the raw cards stay behind the disclosure")
        #expect(view.showsBoundary == false, "task_clarify is backend phase structure")
    }

    /// task_pursue: the red test is not evidence. It stays on the surface.
    @Test func actionableFailureSurvivesTheCollapse() throws {
        let pursue = try turn(
            id: "turn-3",
            text: "Projection landed. One test caught a stale signature; fixed and green.",
            items: [
                edit("f1", "swift/Loopflow/Models/WaveChatTranscript.swift"),
                command("c1", "cargo build"),
                command("c2", "swift test", exit: 1),
                tool("t2", "Edit"),
                command("c3", "swift test"),
            ],
            body: provenance(step: "task_pursue")
        )

        let view = turnPresentation(pursue, audit: false)
        #expect(view.failures.count == 1)
        #expect(isActionableFailure(view.failures[0]))
        #expect(view.activity?.label == "3 commands · 1 file edited · 1 tool call")
    }

    /// A command the harness called `completed` while the shell said no is
    /// still a failure a human has to act on.
    @Test func nonzeroExitIsAFailureEvenWhenTheHarnessSaysCompleted() {
        #expect(isActionableFailure(command("c", "swift test", exit: 1, status: .completed)))
        #expect(!isActionableFailure(command("c", "swift test", exit: 0, status: .completed)))
        #expect(isActionableFailure(tool("t", "Edit", status: .failed)))
    }

    /// A running turn that has not spoken yet still reads as motion, so a
    /// tool-only span never looks like the wave went quiet.
    @Test func aWorkingTurnSaysSo() throws {
        let running = try turn(
            id: "turn-4",
            status: .running,
            items: [tool("t0", "Read"), command("c0", "cargo test")],
            body: provenance(step: "task_pursue")
        )
        let view = turnPresentation(running, audit: false)
        #expect(view.activity?.isRunning == true)
        #expect(view.activity?.label == "Working — 1 command · 1 tool call")
        #expect(!view.hasProse)
    }

    /// Audit is the old view: every card, the flow-step boundary, nothing
    /// coalesced.
    @Test func auditReturnsTheExecutionLog() throws {
        let pursue = try turn(
            id: "turn-3",
            text: "Projection landed.",
            items: [command("c1", "cargo build"), command("c2", "swift test", exit: 1), tool("t2", "Edit")],
            body: provenance(step: "task_pursue")
        )
        let view = turnPresentation(pursue, audit: true)
        #expect(view.auditItems.count == 3)
        #expect(view.activity == nil, "the cards carry their own status")
        #expect(view.showsBoundary)
    }

    /// Prose the harness emitted as an item reads with the turn's speech, not
    /// as a card. Thoughts never reach the eye.
    @Test func harnessProseJoinsTheSpeechAndThoughtsDoNot() throws {
        let reporting = try turn(
            id: "turn-5",
            text: "Opened the PR.",
            items: [
                .message(id: "m0", text: "Waiting on CI.", phase: "task_mutate"),
                .thought(id: "h0", text: "should I rebase?"),
            ]
        )
        let view = turnPresentation(reporting, audit: false)
        #expect(view.prose == "Opened the PR.\n\nWaiting on CI.")
        #expect(view.activity == nil, "a thought is not evidence of work")
    }

    /// Decisions and reports are conversation; lifecycle churn is not.
    @Test func decisionsStayAndChurnGoes() {
        #expect(isConversational(childActivity(.decisionRequired)))
        #expect(isConversational(childActivity(.pullRequestOpened)))
        #expect(isConversational(childActivity(.failed)))
        #expect(isConversational(childActivity(.completed)))
        #expect(!isConversational(childActivity(.stateChanged)))
        #expect(!isConversational(childActivity(.controlApplied)))
    }

    /// The whole transcript: what a human sees of a long-running task, and what
    /// the audit toggle brings back.
    @Test func theVisibleThreadIsSpeechFailuresAndDecisions() throws {
        let thread = [
            try turn(id: "turn-1", role: .user, text: "make wave chat human-first"),
            try turn(
                id: "turn-2",
                text: "Collapsing the cards behind an audit toggle.",
                items: [tool("t0", "Read"), command("c0", "git log")],
                body: provenance(step: "task_clarify")
            ),
            // An empty span at a step boundary: nothing said, nothing done.
            try turn(id: "turn-3", body: provenance(step: "task_pursue")),
            try turn(id: "turn-4", role: .user, status: .completed, from: "project wave-chat",
                     activity: childActivity(.stateChanged)),
            try turn(id: "turn-5", role: .user, status: .completed, from: "project wave-chat",
                     activity: childActivity(.decisionRequired)),
            try turn(
                id: "turn-6",
                text: "PR is up.",
                items: [command("c1", "lf pr open")],
                body: provenance(step: "task_mutate")
            ),
        ]

        let visible = thread.filter { isVisibleTurn($0, audit: false) }.map(\.id)
        #expect(visible == ["turn-1", "turn-2", "turn-5", "turn-6"],
                "the empty span and the lifecycle churn never reach the eye")

        let audited = thread.filter { isVisibleTurn($0, audit: true) }.map(\.id)
        #expect(audited == thread.map(\.id), "audit hides nothing")
    }
}
