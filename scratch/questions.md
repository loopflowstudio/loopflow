# Open questions

## LOO-376: ship the app without local symbols?

The first launch of a newly installed app spends about 390 ms more before
`main` than later launches, and the system charges part of it by executable
size (`scripts/benchmarks/desktop-performance/20261005-first-launch/`).
`strip -x` takes the executable from 28.5 to 17.5 MB and saved 40–100 ms of
that in two loaded-host comparisons.

Not shipped. Crash reports on the machine name functions from those symbols.
Keeping them useful means `dsymutil` before stripping (2.7 s, 83 MB) and a
place that keeps each release's dSYM; the DMG is built on the cron host and the
release's artifact set is fixed. Jack Heart has not chosen between the launch
time and the retained symbols, or where a dSYM would live.

Assumption made: the 400 ms first-frame budget is judged on a launch of a
binary the system has already run; a post-update launch is reported apart.
