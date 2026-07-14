//! Baseline for the Wave Chat token path's listener stage: what one provider
//! text delta costs the wave runtime, and how that cost moves as the open turn
//! grows.
//!
//! Run against a real journal to measure the living workspace, not a demo:
//!
//! ```text
//! cargo bench --bench wave_stream                       # synthetic journal
//! LF_BENCH_JOURNAL=~/src/loopflow/.lf/journal/waves/product/journal.jsonl \
//!   cargo bench --bench wave_stream                     # accumulated transcript
//! ```
//!
//! The listener is the stage between the provider and every reader: it folds
//! the delta into the open turn, appends it to the journal, and broadcasts the
//! turn to SSE subscribers. Each measurement below is one `TurnText` delta
//! applied through `apply_resident_delta`, exactly as `POST /resident/deltas`
//! applies it in the server.

use std::fs;
use std::path::PathBuf;
use std::time::Instant;

use loopflow::wave::runtime::WaveRuntime;
use loopflow::wave::wire::ResidentDelta;

/// A turn's worth of deltas, sized from the real product journal: 657 deltas
/// carrying 3149 characters — a mean of ~4.8 characters per delta.
const DELTAS: usize = 657;
const DELTA_TEXT: &str = " streaming";

fn main() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let repo = tmp.path().to_path_buf();
    let journal = repo.join(".lf/journal/waves/bench/journal.jsonl");
    fs::create_dir_all(journal.parent().expect("parent")).expect("journal dir");

    let seeded = seed_journal(&journal);
    println!("== transcript");
    println!("  journal events   {}", seeded.events);
    println!("  journal bytes    {}", seeded.bytes);

    // Hydration: what a reader waits through before the first token can show.
    let start = Instant::now();
    let runtime = WaveRuntime::open("bench".into(), repo).expect("open runtime");
    let hydrate = start.elapsed();
    println!("  open (hydrate)   {:.1} ms", hydrate.as_secs_f64() * 1e3);

    // One live SSE subscriber, so every delta pays the broadcast serialization
    // the wire pays.
    let mut sub = runtime.subscribe_with_snapshot();
    println!("  thread turns     {}", sub.turns.len());

    runtime.apply_resident_delta(ResidentDelta::TurnOpened {
        answers: Vec::new(),
    });

    let mut samples = Vec::with_capacity(DELTAS);
    let mut wire_bytes = 0usize;
    let turn_start = Instant::now();
    for _ in 0..DELTAS {
        let start = Instant::now();
        runtime.apply_resident_delta(ResidentDelta::TurnText {
            text: DELTA_TEXT.to_string(),
        });
        samples.push(start.elapsed().as_secs_f64() * 1e6);
        wire_bytes += sub
            .turn_rx
            .try_recv()
            .map_or(0, |frame| frame.json.len());
    }
    let turn_elapsed = turn_start.elapsed().as_secs_f64();
    let turn_chars = DELTAS * DELTA_TEXT.len();

    println!("\n== listener cost per text delta ({DELTAS} deltas, {turn_chars} chars)");
    report("first 10%", &samples[..DELTAS / 10]);
    report("last 10%", &samples[DELTAS - DELTAS / 10..]);
    report("all", &samples);

    println!("\n== turn totals");
    println!(
        "  listener time    {:.1} ms ({:.0} deltas/s)",
        turn_elapsed * 1e3,
        DELTAS as f64 / turn_elapsed
    );
    println!(
        "  broadcast bytes  {wire_bytes} for {turn_chars} chars of prose ({:.0}x)",
        wire_bytes as f64 / turn_chars as f64
    );
}

struct Seeded {
    events: usize,
    bytes: u64,
}

/// Copy the journal named by `LF_BENCH_JOURNAL` — the accumulated transcript
/// the KR demands we measure on — or fall back to an empty one.
fn seed_journal(dest: &PathBuf) -> Seeded {
    let Some(src) = std::env::var_os("LF_BENCH_JOURNAL").map(PathBuf::from) else {
        fs::write(dest, "").expect("empty journal");
        return Seeded { events: 0, bytes: 0 };
    };
    let body = fs::read_to_string(&src).expect("read LF_BENCH_JOURNAL");
    fs::write(dest, &body).expect("seed journal");
    Seeded {
        events: body.lines().count(),
        bytes: body.len() as u64,
    }
}

fn report(label: &str, samples: &[f64]) {
    let mut sorted = samples.to_vec();
    sorted.sort_by(|a, b| a.partial_cmp(b).expect("no NaN"));
    let pct = |p: f64| sorted[((sorted.len() as f64 * p) as usize).min(sorted.len() - 1)];
    println!(
        "  {label:<10} median {:>8.1} us   p90 {:>8.1} us   max {:>8.1} us",
        pct(0.5),
        pct(0.9),
        pct(1.0)
    );
}
