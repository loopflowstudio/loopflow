# UX Experiments

Repeatable workflows to test every iteration. Run these manually, note friction, track improvement over time.

## Personas

**The Conductor**
- Has multiple waves running
- Checks in periodically
- Connects for interactive steps, reviews output, lands PRs
- Wants: glanceable status, quick connect, minimal friction to land

**The Improviser**
- Starting fresh on a problem
- Doesn't know the right flow yet
- Runs steps manually, iterates on direction
- Wants: quick wave creation, easy step running, low commitment

**The Listener**
- Was away (hours/days)
- Waves ran while gone, some need attention
- Wants: catch up quickly, see what happened, decide what to do

## Experiment Scripts

Run these against the app. Time each. Note friction points.

### E1: Quick debug (Improviser)

```
1. Copy an error to clipboard
2. Open Concerto
3. Create wave for src/
4. Run "debug" step with clipboard
5. See output, iterate if needed
6. Done

Target: < 30 seconds to running step
```

### E2: Morning check-in (Listener)

```
1. Open Concerto
2. See which waves need attention
3. For each waiting wave: connect, review, continue or fix
4. Land any ready PRs
5. Check running waves are healthy

Target: Status clear in < 5 seconds, each wave handled in < 2 min
```

### E3: Ship a feature (Conductor)

```
1. Have a wave running "ship" flow
2. Get notification: interactive step waiting
3. Connect from Concerto
4. Work with agent, hit Continue
5. Watch next steps run
6. Land when PR ready

Target: Connect → working in < 10 seconds
```

### E4: Start a new project (Improviser)

```
1. Have an idea, no existing wave
2. Create wave, pick area
3. Run "design" step with prompt
4. Iterate on design
5. Run "implement"
6. Add stimulus to keep it running

Target: Idea → running design in < 1 minute
```

### E5: Multi-wave management (Conductor)

```
1. Have 3+ waves in various states
2. Triage: which need me, which are fine
3. Handle the urgent ones
4. Pause or stop ones that are stuck
5. Land ones that are ready

Target: Full triage in < 5 minutes for 5 waves
```

## Tracking

After each iteration, run experiments and record:
- Time to complete
- Friction points (where did you hesitate, get lost, click wrong thing)
- Missing affordances (what did you want that wasn't there)
- Broken flows (what didn't work at all)

## Capture Mechanics

Options for capturing experiment runs:

**Low-tech: Stopwatch + notes**
- Run experiment, phone timer running
- Voice memo or quick notes
- Log time + friction in spreadsheet

**Screen recording + timestamps**
- Start recording, run experiment
- Note timestamps when you hit friction
- Review video at those points only

**Instrumented app**
- Concerto logs timing data internally
- Automatic timing, manual friction notes

**Structured form per experiment**
```
E1: Quick Debug
□ Create wave    __s  friction: ________
□ Run step       __s  friction: ________
□ See output     __s  friction: ________
Total: __s
```
