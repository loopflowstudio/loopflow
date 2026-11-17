import Loopflow

extension InteractionInput {
    @MainActor
    static var current: InteractionInput? { InteractionTrace.input }
}
