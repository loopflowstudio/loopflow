# Review artifacts & shared infrastructure notes

Working notes for `/explore` on review sheets and reusable human-comprehension infrastructure across `explore`, `concept-review`, `qa`, `demo`, `tour`, and `ship-review`.

## Exact words captured

- **Jack Heart:** "the other thing i think ship-review's artifacts kinda suck right now"
- **Jack Heart:** "/explore this is another place where can we look at what's out there in like t3 code, gstack, poteto, simon willison, matt pocock, devin, coderabbit, etc and borrow the best of those to make a really nice review sheet that doenst just feel like every other autogen'ed llm page / 3 made up demo artifacts to aim for"
- **Jack Heart:** "Yes, but it also doesnt seem to be aimed at guiding my attention, but instead like filling out paper work / doesnt put itself in the mind of the human who is both trying to figre out / - did this build the right thing / - is it time to move on or time to iterate / - what do i need to understand about how this works so i can make better decisions and build on top of it goiing forward / i know there are other startups out there claiming to build the future of code review, we should reserach them too"
- **Jack Heart:** "and finally, we should try to figure out to make it kind of reuseable infrastrucutre, where things like concept-review, shi-review, explore, qa, tour-building etc all build on each other, share prompts and infrastructure ot not suck and work together to create a holistic experience for the human coder"
- **Jack Heart:** "as well as be set up to be the right generi thing that you can then build your own more specific prompts on top of / and to be clear, teh companies having AIs review your code are exactly the wrong direction. we want ideas for how to make it easier for humans to cognitively integrate teh code produces by AIs"
- **Jack Heart:** "ryan saxe's pair is another failry simple setup in this veign"
- **Jack Heart:** "should focus on questions liek / what did we build / how does it work / the key data structures and APIs / what i need to know about the techincal design to make informed decisions going forward / shoudl not do thing liek: / here are the 95325 tests we ran that passed / what shoudl we do about the CI checks that are failing that will get fixed by land --wait-and-fix"
- **Jack Heart:** "ok, i like A's simple flow, but i also like the idea of having the pair-style interactability that we have on the diff where you can highlight to comment and then submit questions to the llm and then also adding some sort of global facility for clicking buttons to express choices etc"
- **Jack Heart:** "this should probably be a follow up task to 1104"
- **Jack Heart:** "we could consider having the html know how to send commands / steers so that you dont need the desktop and the expeirencde works in rex"
- **Jack Heart:** "with say an agent-browser split"
- **Jack Heart:** "i dont want to make the esktop experience worse to support rex, but it woudl be nice oth ave both"
- **Jack Heart:** "for what html can trigger, since its loopback only, i dont think its relaly any security hole to just allow any thing on lf"
- **Jack Heart:** "its basically another proxy for the lf machine server interface / is that wrong?"
- **Jack Heart:** "1b probably, but open to either 2b -- what does verify even do"
- **Jack Heart:** "verify: yeah, and run it on auto mode"
- **Jack Heart:** "1b: yes, and open it in rex / 1b should be stacked on 1104 / maybe both should"

## Chosen direction

- Vision A's simple technical flow (what was built, how it works, key data structures & APIs, technical design going forward; no passing test counts or routine CI checks).
- Shared global interaction runtime (highlight-to-comment + `data-choice` / `data-multiselect` / `data-question` / `data-lf`) + `lf page build` kit with real `git diff` injection, repo tokens, and `.lf/components/` overrides.
- Host-agnostic transport: loopback-only HTTP/WS `Carrier` in `lf server` exposing the Machine server `Operation` interface for Rex agent-browser splits (capability token + same-origin check), plus Desktop `TaskHTMLView` bridge into `ReviewCommentsStrip` / `addToConversation` without degrading Desktop, plus offline `file://` clipboard fallback.
- Latest prototype: `scratch/explore.html` (round 3).

## Current exploration page

- `scratch/explore.html` (round 3: host-agnostic loopback `lf server` transport for Rex agent-browser split + Desktop `TaskHTMLView` bridge + offline `file://` fallback, with live host switcher in the prototype and steer-only vs allowlisted-actions fork)
