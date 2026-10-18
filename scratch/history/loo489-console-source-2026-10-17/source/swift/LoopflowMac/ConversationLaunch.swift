/// An independent conversation in the Task's prepared checkout.
/// Repository and Wave entry use their retained primary conversations.
struct TaskConversationLaunch {
    let task: String

    var prompt: String {
        "Help me work on this Task using its directive and current evidence."
            + "\n\nA human is here for a conversation. Briefly orient to this scope, then ask what they want to work on. Opening this conversation does not request an autonomous operating pass, a new Task or worktree, or a message to anyone else."
    }

    func arguments(lf: String) -> [String] {
        [lf, "--interactive", "--task", task, ":", prompt]
    }
}
