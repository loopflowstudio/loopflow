# Reading a Desktop-sized terminal

A phone observer can pan upward within a taller remote grid before reaching its scrollback. Test that path through native gestures and inspect the actual numbered lines in screenshots. A stable outer contentOffset cannot establish that the text stayed still: the inner terminal may still follow output.

Add a 120×80 option only to the explicit offline fixture. Supply output through its existing file source, open passively, swipe into the screen, append output, and verify the line in the middle remains visible. OCR reads the rendered numbered lines, not model state or scroll percentage. Verify Latest reveals the final line without taking control. The test itself needs a run before making claims about the product.

Before repair, the native check failed19.592s; screenshots changed from lines90–123 to170–203 after80 new lines. Outer scrolling was not recorded as reading intent, so the inner terminal kept following output. Source1a14a25d8 records deliberate scrolling in either view and restores the inner row if feed advances it while reading.46 terminal tests passed8.656s; three native scroll tests passed56.691s. Screenshot inspection confirms the same lines stayed visible but found a one-row alignment shift: synchronizing an already-bottom inner offset floors its partial-cell maximum. A follow-up leaves that correct logical tail row alone; three native checks are running against that refinement. This is not physical-phone or output-during-touch proof.

Fresh lf sync --manual completed with no source change. LOO-718 status read succeeded by full identifier: active, feature/demo, PR1812 open and unarmed. Bare718 failed resolution; it did not establish a missing Task or service.

Tail-refinement run: three native checks passed54.909s (Desktop20.761s, streamed reading18.615s, swipe/Latest15.533s), runner29342 exited0. Inspected desktop-scroll-tail screenshots show lines91–124 before and after, with only slight pixel alignment; the after image contains an OS Apple Intelligence notification above the terminal. Signed device build also passed, but this artifact has not been retained/installed yet. An additional check now covers replying after scrolling back, because SwiftTerm returns to its caret on input and the local reading flag must not override that transition.

Reply-after-history regression failed23.047s: the native terminal returned to the caret, but the controller kept its reading flag and froze subsequent output. Before the next feed, clear that flag when both scroll views are now at the bottom; share the same visible-position predicate with Latest and scroll callbacks. Final checks in progress.

Final source84bbc89cbda6bf7061b991772dc953421ceebbe6:46 terminal checks passed8.505s and all three native regressions passed (Desktop19.422s, streaming18.149s, swipe/Latest/reply21.074s). Runner8396 exited0 and removed its Simulator. Signed phone build and strict signature passed, installed through CoreDevice17:20:57 local. Final reply screenshot inspected: line120 and prompt visible, no Latest. Artifacts reviewroot/desktop-scroll-final. Fresh sync changed no source. Normal launch requested; physical scrolling on the new build remains unverified.

Normal physical app launch succeeded17:21:19 local, no fixture arguments. This proves launch, not physical gesture acceptance.
