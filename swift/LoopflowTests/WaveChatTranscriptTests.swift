import Testing
@testable import Loopflow

// The information hierarchy, pinned against a realistic long-running `task`
// flow: clarify reads the repo and writes a design note, pursue builds and hits
// a red test, mutate opens the PR, and a child project asks for a decision.
//
// What a human must see: the wave's prose and the decision. What must disappear:
// twenty-odd tool calls, shell commands, file edits, thoughts, and the
// `task_clarify` / `task_pursue` / `task_mutate` phase structure.

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
    @Test func machineryDoesNotEnterTheConversation() throws {
        let clarify = try turn(
            id: "turn-2",
            text: "The transcript renders every tool call as a card. I'll keep them out of the conversation.",
            items: [
                .thought(id: "h0", text: "weighing the options"),
                tool("t0", "Read"),
                tool("t1", "Grep"),
                command("c0", "git log --oneline -3"),
                edit("f0", "scratch/w2-129.md"),
            ],
            body: provenance(step: "task_clarify")
        )

        let view = turnPresentation(clarify)
        #expect(view.prose.hasPrefix("The transcript renders"))
        #expect(view == TurnPresentation(prose: clarify.text))
    }

    /// task_pursue: an expected red test is execution evidence, not another
    /// command card in the human conversation.
    @Test func backendFailuresDoNotBecomeChatCards() throws {
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

        let view = turnPresentation(pursue)
        #expect(view == TurnPresentation(prose: pursue.text))
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
        #expect(!turnPresentation(running).hasProse)
        #expect(isVisibleTurn(running))
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
        let view = turnPresentation(reporting)
        #expect(view.prose == "Opened the PR.\n\nWaiting on CI.")
    }

    /// Real Product Wave turn (journal turn-141): the harness tagged its process
    /// narration `commentary` and its outcome `final_answer`. Curation surfaces
    /// the outcome and folds the narration into steps; the raw prose keeps both.
    @Test func commentaryFoldsIntoStepsAndFinalAnswerLeads() throws {
        let structured = try turn(
            id: "turn-141",
            items: [
                .message(
                    id: "m0",
                    text: "I’m using `wave_clarify` to finish the interrupted audit: verify the "
                        + "product charter, project KRs, executable cadence, and live backlog.",
                    phase: "commentary"
                ),
                .message(
                    id: "m1",
                    text: "The prior diagnosis holds: the charter describes a daily product "
                        + "dogfood loop, but `crons: []` makes that cadence inert.",
                    phase: "commentary"
                ),
                .message(
                    id: "m2",
                    text: "Clarification complete.\n\n- Activated the daily product wave cron "
                        + "at 08:00.\n- Replaced `N/N` placeholders with 5/5 trials.",
                    phase: "final_answer"
                ),
            ],
            body: provenance(step: "wave_clarify")
        )

        let view = turnPresentation(structured)
        #expect(view.conclusion.hasPrefix("Clarification complete."))
        #expect(view.steps.count == 2)
        #expect(view.hasSteps)
        // Nothing is lost: the raw prose still carries the narration.
        #expect(view.prose.contains("wave_clarify"))
        #expect(view.prose.contains("Clarification complete."))
    }

    /// The honest boundary: a streaming turn arrives as one merged `turn.text`
    /// blob (the wire folds every `stream` fragment together), so it cannot be
    /// structurally split. It reads in full as its conclusion, with no steps.
    @Test func streamingProseStaysWholeWithNoSteps() throws {
        let streaming = try turn(
            id: "turn-22979",
            text: "I’m using `wave_mutate` to judge the accepted human controls. "
                + "The receipts show both are incorporated. Product’s objective, "
                + "portfolio, KRs, and memory remain unchanged.",
            items: [command("c0", "lf pm show")],
            body: provenance(step: "wave_mutate")
        )

        let view = turnPresentation(streaming)
        #expect(view.conclusion == streaming.text)
        #expect(view.steps.isEmpty)
        #expect(!view.hasSteps)
    }

    /// Decisions and reports are conversation; lifecycle churn is not.
    @Test func decisionsStayAndChurnGoes() {
        #expect(isConversational(childActivity(.decisionRequired)))
        #expect(isConversational(childActivity(.prOpened)))
        #expect(isConversational(childActivity(.failed)))
        #expect(isConversational(childActivity(.completed)))
        #expect(!isConversational(childActivity(.stateChanged)))
        #expect(!isConversational(childActivity(.controlApplied)))
        #expect(!isConversational(childActivity(.directed)))
        #expect(!isConversational(childActivity(.incorporated)))
    }

    /// The whole transcript: what a human sees of a long-running task.
    @Test func theVisibleThreadIsSpeechAndDecisions() throws {
        let thread = [
            try turn(id: "turn-1", role: .user, text: "make wave chat human-first"),
            try turn(
                id: "turn-2",
                text: "Collapsing the cards into one activity line.",
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

        let visible = thread.filter(isVisibleTurn).map(\.id)
        #expect(visible == ["turn-1", "turn-2", "turn-5", "turn-6"],
                "the empty span and the lifecycle churn never reach the eye")
    }
}
