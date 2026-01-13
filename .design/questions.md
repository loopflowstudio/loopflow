# Open Questions

Questions that need answers before implementation.

## Scope

1. **What does "onboarding" mean here?** I've assumed it's about improving the first-run experience. Is this correct, or is there a specific feature you have in mind?

2. **Which option?** The design proposes three approaches:
   - A: First-run wizard (intercepts any command)
   - B: Improved `lf ops init` (explicit)
   - C: Status-aware help (passive hints)

   Which direction do you prefer?

## Behavior

3. **Should setup be automatic or prompted?** When dependencies are missing, should we:
   - Automatically install them (current `lf ops install` behavior)
   - Ask first ("Install missing dependencies? [Y/n]")
   - Just report what's missing and tell them to run `lf ops install`

4. **What about non-macOS?** Current code is macOS-only. Should onboarding:
   - Fail gracefully with instructions for manual setup
   - Try to work with whatever's available
   - Something else

## Context

5. **Is there prior art or a specific workflow you're modeling?** Understanding what inspired this would help nail the UX.
