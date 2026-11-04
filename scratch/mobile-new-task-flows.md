# Starting work from the phone

Jack Heart requested three interactive prototypes of new-Task flows on November 3, 2026. This explores an additional high-priority capability alongside LOO-718's existing-Task MVP; it does not create a real Task or select an implementation.

Open [the three prototypes](explore.html). Each starts from the same working-Task list and ends in an illustrative Session terminal. Desktop typography and Light/Dark colors are retained. No Wave selection or extra panes are required by these studies.

1. **Describe first.** A compact creation sheet accepts typed or dictated instructions. The last-used repository and Machine are visible defaults, editable before Create & start. Fewest decisions for familiar work; the risk is overlooking the destination.
2. **Choose the workspace first.** Select a repository/Machine pair before composing. The destination stays visible throughout. More deliberate for multiple environments; an extra step for frequent work in the same repository.
3. **Talk it through.** Capture an idea before choosing a destination, then answer one missing-context question and review an editable brief. This explores a voice-led conversation that organizes work. It requires reliable interpretation and can add latency or distort intent; the original words remain visible and nothing starts without explicit action.

All three preserve draft input through back navigation within the page and allow typing instead of simulated speech. Browser refresh resets the study. Every Create & start action is a local simulation, with no microphone, network, provider or tracker access. Dictation uses a fixed example; route suggestions in the conversational version are fixtures, never real inferred facts. Actual keyboard geometry, speech quality, permissions, Task creation semantics and uncertain delivery still require native implementation proof.

Jack Heart selected flow 1, **Describe first**, after reviewing the prototypes: “1 looks good.” Carry forward the compact sheet, editable typed/dictated request and visible changeable destination. This selects the interaction direction; native speech implementation, creation delivery guarantees and exact launch semantics remain to design. The other flows remain comparison studies.

Verification: Playwright passed all three simulated creation paths, empty-input guards, dictation, destination changes, draft and edited-brief preservation, return to the created Session, typing alternative, and 320/390/800/1440px Light/Dark overflow checks. Desktop and narrow captures inspected. Native speech, keyboard and creation behavior remain unproved.
