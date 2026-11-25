# LOO-1094 demo notes

## Reactions

- Jack Heart: "shoudl the cancel not be the red button?"
- Jack Heart: "im not sure about the red abandon link" / "any prior ar there?"
- Jack Heart: "i mean in other products"
- Jack Heart: "can i see the linear and asana treatments for this"
- Jack Heart: "button should be green i think? otherwise variant a works"
- Jack Heart: "the ellipsis isnt doing any thing"
- Jack Heart: "lg2m ship it"

## What happened

In `TaskCompleteConfirmation` (`swift/LoopflowMac/Views/TaskWorkView.swift`), SwiftUI's `.confirmationDialog` automatically promotes `Cancel` to the default button whenever another button has `role: .destructive`. Because `LoopflowApp` sets `.tint(theme.palette.accentInk)` (burgundy/red), macOS fills `Cancel` with the red/burgundy accent right underneath the red/pink `Complete & Abandon Children` button.
