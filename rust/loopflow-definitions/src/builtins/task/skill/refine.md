---
produces: refined text
action_style: exploratory
---
Refine text by offering contrasting options and learning from the choices.

The goal is to learn enough about the text's purpose, audience and voice to
make the whole of it more true. Each session starts fresh: don't assume
preferences and don't project a style. Match the preferences shown, not generic
"good writing". Meaning first, then expression.

Before editing, pin down who reads it and what it should let them do, and which
facts, terms, examples, links and code must survive. Then watch for the ways
this goes wrong: smoother prose that changes meaning, a local choice that makes
the whole inconsistent, claims the source can't back, polish that erases the
author's voice.

## Method

1. **Find the text.** A file path (`lf refine README.md`), clipboard content
   (`-v`), or ask.

2. **Diagnose the axis.** What kind of improvement does it need?
   - **Structure**: order, splits, merges
   - **Voice**: too formal, too casual, inconsistent
   - **Ideas**: missing points, wrong emphasis, unclear purpose
   - **Positioning**: wrong audience, unclear value
   - **Density**: too verbose or too terse

   Say it with the reason: "This is a structure problem: the intro repeats
   what comes later." Don't offer word choices when the real problem is
   section order.

3. **Work section by section.** Take a paragraph or a few sentences and offer
   two or three options that each make a real tradeoff on the axis. Ask which,
   and why: "I like B" teaches nothing; "I like B because it shows the flow"
   does. Don't signal a favorite first.

4. **Transfer what you learned.** After a few sections, state the pattern
   ("You lead with the insight, and cut sections that exist only for
   structure") and try it on a fresh section. A choice that contradicts your
   model means the model is wrong or too broad; revise it, don't explain the
   choice away.

5. **Full pass.** Apply the preferences to the rest, then reread the whole
   against every fact that must survive and every stated preference. Read it
   once as the audience. Present the complete result.

## Options

Each option is a different approach, not wordsmithing:

```
**Original:**
The system processes requests by evaluating them against the configured rules.

**Option A:** (more specific)
When a request arrives, the system checks it against each rule in config.yaml, rejecting on first match.

**Option B:** (more conversational)
Requests get filtered through your rules—first match wins, and anything that passes goes through.
```

If every option shares the same hidden assumption, the contrast teaches
nothing.

Stop when they say it's good enough, when remaining differences are cosmetic,
or when preferences conflict: name the tension and ask. With nobody to choose,
generate the options yourself, pick the strongest for the audience and the
repository's voice, and note any open tradeoff.

Leave the refined text in the original file, or on the clipboard for pasted
content. Don't over-polish.
