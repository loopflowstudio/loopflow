# Native multiline insertion · November10

The pinned SwiftTerm dictation-result implementation calls `insertText`. Tested that boundary with LF-separated text: entire draft retained, no submission, subsequent delete/insert worked, and Return submitted once. Production unchanged;1 native test passed0.192s. The deterministic editor treats unbracketed LF as text and CR as submit, so this result establishes neither other providers’ line-ending behavior nor microphone/physical dictation quality. No production defect established and no input workaround added. Regression retained to cover native multiline insertion separately from the Paste button.

Log: reviewroot/multiline-input/native-insertion.log. Runner21046 exited0 and removed its Simulator.
