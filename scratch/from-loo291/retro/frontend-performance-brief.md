# Snappiness — what matters for Loopflow's users

2026-09-26. The one-page version of [frontend-performance.md](frontend-performance.md)
for Jack. Numbers there; judgment here.

## What our users actually feel

Loopflow is a cockpit that stays open all day while agents work. That shapes
which milliseconds matter:

1. **Idle must be silent.** The app is on screen for hours doing nothing
   visible. Any stutter while idle reads as "this app is heavy" and taxes every
   other app on the machine. This is the moment we are worst at today.
2. **Clicks must feel instant.** Wave → Task → Session is the whole navigation
   model. If a click takes longer than a blink, people stop drilling down and
   start keeping terminals open on the side — which defeats the product.
3. **Typing in a Session must feel like a terminal.** This is the only place
   we compete head-on with Ghostty, Warp and iTerm. We embed Ghostty, so we
   inherit its speed; we can only lose it by putting our own work between the
   key and the glyph.
4. **Launch can take a second; it must not take five.** Jack opens the app a
   few times a day. Launch is a first impression, not a habit.

Everything else — Markdown rendering, comment threads, 50-Task plans — only
matters insofar as it leaks into those four.

## Where we stand

| Moment | Today | Verdict |
|---|---|---|
| Idle, nothing clicked | 10 stutters in 6 s on the installed app; eight half-second freezes over 45 s | **Failing.** Caused by our own 2 s polling, which re-rendered the whole window with identical data. |
| Launch to usable workspace | ~110 ms to rows at 200 Tasks in the harness; real-machine number not yet taken | Fine. |
| Click Wave / Task / Session | Not yet measured against its budget (100 ms) | Unknown; likely fine at today's size, at risk at 50 Tasks per Wave. |
| Keystroke in a Session | Not yet measured; Ghostty draws on its own thread | Expected fine; we now have the instrument to prove it. |
| Memory over a day | Flat over 45 s idle; navigation-loop test blocked on a fixture gap | Unknown. |

## What we did about it

- Fixed the idle re-render at its root: the app now only republishes data
  that changed. Two lines of intent, the biggest single win available.
- Built the ruler. The app now names its own moments (launch, click, keystroke,
  each `lf` read) in a way macOS records for free, with zero cost when nobody
  is looking. One script records a normal hour of use on Jack's machine and
  reports it; one test suite measures the same moments on synthetic data in
  CI. No telemetry, nothing leaves the machine — by design, and it stays that
  way for customers.

## What we should do next, in order

1. **Stop polling; stream.** Every 2 s the app launches two copies of a 47 MB
   binary to ask "anything new?". Runs already use a live stream; Sessions
   should too. This removes the remaining idle work and the freezes.
2. **Take the real numbers.** Record an ordinary hour on Jack's machine with
   the instrumented build. Until then the click and typing verdicts are
   expectations, not facts.
3. **Only then optimize clicks**, and only what the recording shows: the
   sidebar rebuilds its whole model on every draw and the Flow diagram lays
   itself out on every visit. Both are cheap to cache if they turn out to
   matter.

## What we are not doing

No telemetry. No performance work ahead of the visual pass Jack is reviewing
now — snappiness is the second pass, and the ruler exists so that pass is
measured rather than felt.
