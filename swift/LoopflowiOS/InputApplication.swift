import Loopflow
import UIKit

/// Scope the original event clock to dispatch. Never retain a last-event guess
/// for a later timer, subscription, or programmatic navigation.
@objc(LoopflowInputApplication)
final class InputApplication: UIApplication {
    override func sendAction(_ action: Selector, to target: Any?, from sender: Any?, for event: UIEvent?) -> Bool {
        guard let event else { return super.sendAction(action, to: target, from: sender, for: event) }
        return InteractionTrace.$input.withValue(InteractionInput(uptime: event.timestamp, source: .uiKitEvent)) {
            super.sendAction(action, to: target, from: sender, for: event)
        }
    }

    override func sendEvent(_ event: UIEvent) {
        let input: InteractionInput? = switch event.type {
        case .touches, .presses: InteractionInput(uptime: event.timestamp, source: .uiKitEvent)
        default: nil
        }
        InteractionTrace.$input.withValue(input) { super.sendEvent(event) }
    }
}
