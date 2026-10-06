# First launch of a new binary, 2026-10-05

```sh
uv run python scripts/benchmarks/desktop-performance/launch.py run --work /tmp/first/full --output /tmp/first/out/full-0 --uncached 1 --saved 0 --failed 0 --first-launch 5
uv run python scripts/benchmarks/desktop-performance/launch.py run --work /tmp/first/strip --strip --home /tmp/first/full/home --output /tmp/first/out/strip-0 --uncached 1 --saved 0 --failed 0 --first-launch 5
```

Both launches the app has recorded on Jack Heart's machine (`timings.py`, 0.13.2
and 0.13.3) spent 790 and 625 ms before `main`; the launch benchmark had always
reported about 35 ms. Each recorded launch was the first run of a newly
installed version. The benchmark opened one bundle again and again.

`saved_first_launch` opens a copy of the bundle at a path never used before,
with a saved workspace. Real launches of the release build of `bee8dcc49` on
that machine (Apple M4 Max, `lf` 0.13.3, a private copy of the Home with a
1698 MB database), milliseconds from kernel process start:

| | samples | before `main` median / p95 | first frame median / p95 |
|---|---|---|---|
| `baseline/` new binary, 28.5 MB | 20 | 427 / 524 | 832 / 931 |
| `stripped/` new binary, 17.5 MB (`strip -x`) | 20 | 389 / 454 | 779 / 878 |
| `baseline/` same bundle again | 4 | 35 / — | 430 / — |
| `stripped/` same bundle again | 4 | 33 / — | 438 / — |

Four rounds of five baseline then five stripped first launches, so both saw
the same host load. Median before `main` per round, baseline → stripped:
427 → 321, 422 → 386, 469 → 398, 457 → 408.

- The first launch of a new binary costs about 390 ms that later launches do
  not, all of it before `main`: from `main` to the first frame is about 400 ms
  either way. A launch after an update cannot meet the 400 ms first-frame
  budget by doing less work in the app.
- The system charges it, in part by file size. A 34 KB C program that only
  prints its age took about 100 ms on its first run and 2 ms on its second;
  the same program with 28 MB of constant data took about 300 ms and 2 ms
  (four copies each).
- It follows the path. Copies at one reused path stopped paying after five to ten
  launches, and a re-signed copy with a new code hash at a new path paid the
  same as an unchanged copy.
- Local symbols and the debug map are 11 MB of the 28.5 MB executable. Without
  them the first launch was 37 ms sooner to `main` at the median here, and
  about 100 ms sooner in an earlier five-round comparison under lighter load
  (421 → 321). Nothing ships stripped: crash reports on the machine name
  functions from those symbols, and the release keeps no dSYM.
- The same bundle opened again reached its first frame at 430 ms (4 samples).

Limits. Load averages ran from 15 to over 300 from unrelated work, so absolute
numbers are pessimistic and the stripped difference is small against the noise.
The bundle is ad-hoc signed; the released app is notarized with a hardened
runtime and also carries the 49 MB `lf`, so its first-launch cost may differ,
and the two recorded real launches are the only evidence of it. What the
system does in that time was not traced. First frame is the commit to the
render server. OS file caches were warm; no quiet-host run is here.
