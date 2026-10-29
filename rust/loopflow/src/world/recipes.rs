//! Named worlds. Each recipe says which rung of the ladder it climbs to and
//! what it is the right venue to demo.

use anyhow::Result;

use super::{Outcome, PrSurface, Threshold, World};

/// Rows: store records through production writers. Artifacts: also files a
/// reader opens, such as provider transcripts. Providers: also answers from
/// the World's stand-ins for outside services, such as GitHub's PR reads.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum Rung {
    Rows,
    Artifacts,
    Providers,
}

impl Rung {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Rows => "rows",
            Self::Artifacts => "artifacts",
            Self::Providers => "providers",
        }
    }
}

#[derive(Debug)]
pub struct Recipe {
    pub name: &'static str,
    pub rung: Rung,
    pub demos: &'static str,
    pub build: fn(&World) -> Result<()>,
}

pub const RECIPES: &[Recipe] = &[
    Recipe {
        name: "one-task",
        rung: Rung::Artifacts,
        demos: "Wave → Task → Session → Graph click-through on one small piece of work; \
                a Task waiting at a Workflow node; an eight-step Graph exec (LOO-392)",
        build: one_task,
    },
    Recipe {
        name: "portfolio",
        rung: Rung::Artifacts,
        demos: "dashboards and Portfolio: several Waves with KRs and Tasks in every state, \
                including a stopped Workflow route and a hand-moved Workflow",
        build: portfolio,
    },
    Recipe {
        name: "lifecycle",
        rung: Rung::Artifacts,
        demos: "Task history: unassigned, abandoned, canceled, standalone and weeks of completions",
        build: lifecycle,
    },
    Recipe {
        name: "density",
        rung: Rung::Artifacts,
        demos: "sidebar density: 150 Tasks across three Waves, 20 started, orphan Sessions",
        build: density,
    },
    Recipe {
        name: "wave-backlog",
        rung: Rung::Artifacts,
        demos: "LOO-514: Wave backlog, conversation and open-Task dashboard together",
        build: wave_backlog,
    },
    Recipe {
        name: "task-review",
        rung: Rung::Providers,
        demos: "LOO-515: brief, changes, files and PR beside the Task conversation",
        build: task_review,
    },
    Recipe {
        name: "two-people",
        rung: Rung::Rows,
        demos:
            "LOO-555: Maya and Lee, each on their own Home, sharing one repository's Git planning",
        build: two_people,
    },
    Recipe {
        name: "wave-lifecycle",
        rung: Rung::Artifacts,
        demos: "LOO-500: adding a Wave and removing empty and populated ones",
        build: wave_lifecycle,
    },
];

pub fn find(name: &str) -> Option<&'static Recipe> {
    RECIPES.iter().find(|recipe| recipe.name == name)
}

fn one_task(world: &World) -> Result<()> {
    world.wave("search")?;
    world.plan(
        "search",
        &[("Session search finds a phrase from any turn", false)],
        &[],
    )?;
    let task = world.task(
        Some("search"),
        "Search Session transcripts",
        "Search matches only titles today. Index transcript turns so a phrase finds its Session.",
    )?;
    let task = world.checkout(&task)?;
    world.session(
        Some(&task),
        "Design transcript search",
        &[
            "Search only matches titles. Can we index the transcript turns?",
            "Yes. Turns already live in each provider transcript; an index keyed by Session can follow them.",
            "Start with Claude transcripts and keep the index rebuildable.",
            "Done: the index rebuilds from transcripts and the list filters by phrase.",
        ],
        0.2,
    )?;
    world.task_run(&task, "feature", 0.03, 8, Outcome::Succeeded)?;
    world.task_run(&task, "pursue", 0.01, 4, Outcome::Succeeded)?;
    Ok(())
}

fn portfolio(world: &World) -> Result<()> {
    for wave in ["desktop", "systems", "quality"] {
        world.wave(wave)?;
    }
    world.metric(
        "desktop",
        "tasks-opened-fast",
        "Tasks open in under 200 ms",
        "graduated",
        Some(42.0),
        1.0,
    )?;
    world.metric(
        "desktop",
        "row-states-distinct",
        "Task states with a distinct row",
        "installed",
        Some(3.0),
        2.0,
    )?;
    world.metric(
        "desktop",
        "demo-worlds-used",
        "Demos run on a built world",
        "installed",
        None,
        0.0,
    )?;
    world.plan(
        "desktop",
        &[
            (
                "Opening a Task shows its conversation in under 200 ms",
                true,
            ),
            ("Every Task state has a distinct row treatment", false),
        ],
        &[
            ("tasks-opened-fast", Threshold::AtLeast { value: 40.0 }),
            ("row-states-distinct", Threshold::AtLeast { value: 4.0 }),
        ],
    )?;
    world.plan(
        "systems",
        &[(
            "Older lf keeps opening the store after additive releases",
            false,
        )],
        &[],
    )?;

    let shipped = world.task(
        Some("desktop"),
        "Show Task rows by state",
        "Rows read the same for every state; make done, running and blocked distinct.",
    )?;
    let shipped = world.checkout(&shipped)?;
    world.session(
        Some(&shipped),
        "Row treatments",
        &[
            "Rows for done and running Tasks look identical.",
            "Done rows dim the title; running rows show the live step.",
        ],
        4.0,
    )?;
    world.task_run(&shipped, "pursue", 3.5, 20, Outcome::Succeeded)?;
    world.complete(&shipped, "Landed", 3.0)?;

    let running = world.task(
        Some("desktop"),
        "Open conversations faster",
        "Opening a Task waits on the full history; show the last turns first.",
    )?;
    let running = world.checkout(&running)?;
    world.session(
        Some(&running),
        "Faster open",
        &[
            "Opening a long Task takes two seconds.",
            "The history loads in full before first paint. Paint the tail first.",
            "Do it.",
        ],
        0.5,
    )?;
    world.task_run(&running, "feature", 0.03, 5, Outcome::Succeeded)?;
    world.wait_at(&running, "demo")?;
    world.uncommitted(
        &running,
        &[(
            "README.md",
            "# Demo project\n\nOpening a Task paints its last turns first.\n",
        )],
    )?;
    world.comment(
        &running,
        "Paint the last twenty turns first; load the rest behind them.",
    )?;

    let failed = world.task(
        Some("systems"),
        "Record a reader floor per migration",
        "Additive migrations should not lock out older lf.",
    )?;
    let failed = world.checkout(&failed)?;
    // `code` sets out on `pursue`, which fails at `sync` (its third step).
    world.task_run(&failed, "code", 0.02, 6, Outcome::FailedAt(2))?;

    world.task(
        Some("systems"),
        "Copy the store for unreleased builds",
        "A build with drafts works on a fresh copy of the real store.",
    )?;
    world.task(
        Some("quality"),
        "Unify Desktop fixtures",
        "mock-waves, WorkFixture and the native fixture become one world builder.",
    )?;
    world.session(
        None,
        "Untracked exploration",
        &[
            "What breaks if a Session has no Task?",
            "Nothing; it lists under the repository.",
        ],
        2.0,
    )?;
    Ok(())
}

fn lifecycle(world: &World) -> Result<()> {
    let wave = Some("release");
    world.wave("release")?;
    world.plan(
        "release",
        &[("Every release ships with its notes the same day", false)],
        &[],
    )?;

    for (title, days, session, removed) in [
        ("Shipped yesterday", 1.0, true, false),
        ("Shipped this week", 5.0, false, false),
        ("Shipped last fortnight", 12.0, false, true),
    ] {
        let task = world.task(wave, title, "Publish the release notes with the tag.")?;
        let task = world.checkout(&task)?;
        if session {
            world.session(
                Some(&task),
                title,
                &[
                    "Draft the notes from the merged PRs.",
                    "Drafted and attached to the tag.",
                ],
                days + 0.2,
            )?;
        }
        world.task_run(&task, "pursue", days + 0.1, 15, Outcome::Succeeded)?;
        world.complete(&task, "Landed", days)?;
        if removed {
            world.remove_checkout(&task)?;
        }
    }
    let quarter = world.task(wave, "Shipped last quarter", "Sign the macOS build in CI.")?;
    world.complete(&quarter, "Landed", 40.0)?;

    let unassigned = world.task(
        wave,
        "Unassigned mid-flight",
        "Move the changelog into the release workflow.",
    )?;
    let unassigned = world.checkout(&unassigned)?;
    world.session(
        Some(&unassigned),
        "Changelog workflow",
        &[
            "Generate the changelog in the release workflow.",
            "Started; waiting on the new tag format before going further.",
        ],
        2.0,
    )?;
    world.task_run(&unassigned, "pursue", 1.8, 20, Outcome::Succeeded)?;
    world.unassign(&unassigned)?;

    let abandoned = world.task(
        wave,
        "Abandoned attempt",
        "Publish nightly builds to a second channel.",
    )?;
    let abandoned = world.checkout(&abandoned)?;
    world.task_run(&abandoned, "pursue", 3.0, 25, Outcome::FailedAt(0))?;
    world.abandon(&abandoned)?;

    let canceled = world.task(
        wave,
        "Canceled upstream",
        "Mirror releases to a second registry.",
    )?;
    let canceled = world.checkout(&canceled)?;
    world.task_run(&canceled, "pursue", 2.5, 15, Outcome::Succeeded)?;
    world.cancel_upstream(&canceled)?;

    let lost = world.task(wave, "Lost checkout", "Retry a failed notarization once.")?;
    let lost = world.checkout(&lost)?;
    world.session(
        Some(&lost),
        "Notarization retry",
        &[
            "Notarization fails about once a week.",
            "Retry once on a transient status; fail on anything else.",
        ],
        1.5,
    )?;
    world.remove_checkout(&lost)?;

    let running = world.task(
        wave,
        "In progress",
        "Attach checksums to every release artifact.",
    )?;
    let running = world.checkout(&running)?;
    world.session(
        Some(&running),
        "Checksums",
        &[
            "Attach a SHA-256 for every artifact.",
            "Computing them in the upload step.",
        ],
        0.2,
    )?;
    world.task_run(&running, "pursue", 0.05, 15, Outcome::Succeeded)?;

    world.task(wave, "Not started", "Announce releases in Discord.")?;

    world.task(
        None,
        "Standalone idea",
        "Try a smaller app icon in the menu bar.",
    )?;
    let fix = world.task(
        None,
        "Standalone fix",
        "The README install line has a typo.",
    )?;
    let fix = world.checkout(&fix)?;
    world.session(
        Some(&fix),
        "README typo",
        &["Fix the install line.", "Fixed."],
        0.3,
    )?;
    Ok(())
}

fn density(world: &World) -> Result<()> {
    const PHRASES: [&str; 4] = [
        "Retry and publish the chapter safely",
        "Understand running work",
        "A calmer desktop workspace",
        "Keep Task observation read-only",
    ];
    for (wave_index, wave) in ["product", "infrastructure", "intelligence"]
        .into_iter()
        .enumerate()
    {
        world.wave(wave)?;
        for index in 0..50 {
            let title = format!("{} {index}", PHRASES[index % PHRASES.len()]);
            let task = world.task(Some(wave), &title, "One of many Tasks in a dense Wave.")?;
            let global = wave_index * 50 + index;
            if global < 140 && global % 7 == 0 {
                world.claim(&task)?;
                world.session(
                    Some(&task),
                    &title,
                    &["Pick this up.", "Started; the first pass is underway."],
                    0.5,
                )?;
            }
        }
    }
    for title in ["demo", "realign", "luna-aria", "loopflow"] {
        world.session(
            None,
            title,
            &["Look around.", "Nothing here belongs to a Task yet."],
            1.0,
        )?;
    }
    Ok(())
}

const PASSING: PrSurface = PrSurface::Published {
    draft: false,
    checks: &[("lint", "SUCCESS"), ("unit", "SUCCESS")],
    review: "APPROVED",
    reviewers: &[],
};

/// Unstarted `desktop` work, ranked in creation order.
const BACKLOG: &[(&str, &str)] = &[
    (
        "Rank the Wave backlog",
        "The backlog lists Tasks in creation order, so the most important idea \
         is wherever it happened to land.\n\nLet a person drag Tasks into rank \
         order and keep that order across restarts. New Tasks join at the bottom.\n\n\
         Done when reordering survives relaunch and the Wave conversation sees \
         the same order.",
    ),
    (
        "Read a brief without opening the Task",
        "Choosing what to start means opening Task after Task to read its brief.\n\n\
         Show the first paragraph under each backlog row and expand the whole \
         brief in place. Keep rows a fixed height when collapsed so the list \
         doesn't jump.",
    ),
    (
        "Start a backlog Task from the Wave conversation",
        "The Wave conversation often ends with \"let's do the search one next\", \
         and then the person goes looking for it.\n\nWhen the conversation names \
         a backlog Task, offer to start it right there. Starting checks it out \
         and opens its conversation; nothing runs until the person says so.",
    ),
    (
        "Group open Tasks by where their work stands",
        "Open Tasks mix running, failed, idle and merged-but-open work in one \
         list.\n\nGroup them: running now, needs a look (failed or waiting), \
         idle, and merged but not completed. Each group shows its count even \
         when collapsed.",
    ),
    (
        "Show the last Graph on each Task row",
        "A Task row says nothing about its last run.\n\nShow the last Graph's \
         name, result and age on the row (\"gate failed · 2h\"). Clicking it \
         opens that Graph's steps.",
    ),
    (
        "Archive stale backlog ideas",
        "Ideas older than a chapter clutter the backlog but shouldn't be lost.\n\n\
         Offer an Archived section, collapsed by default, and a one-click way \
         back. Archiving never deletes a Task.",
    ),
    (
        "Keyboard navigation across the dashboard",
        "Moving between backlog, open Tasks and the conversation needs the mouse.\n\n\
         Arrow keys move within a list, Tab moves between lists, Return opens \
         the selection, and the conversation keeps its own focus.",
    ),
];

fn wave_backlog(world: &World) -> Result<()> {
    for wave in ["desktop", "systems", "research"] {
        world.wave(wave)?;
    }
    world.plan(
        "desktop",
        &[
            ("A Wave's backlog reads as a ranked list of briefs", false),
            (
                "Every open Task shows where its work stands at a glance",
                true,
            ),
        ],
        &[],
    )?;
    for (title, brief) in BACKLOG {
        world.task(Some("desktop"), title, brief)?;
    }

    let recent = world.task(
        Some("desktop"),
        "Paint the conversation tail first",
        "Opening a long Task waits on its full history. Paint the last turns first.",
    )?;
    let recent = world.checkout(&recent)?;
    world.session(
        Some(&recent),
        "Tail-first paint",
        &[
            "Opening a long Task takes two seconds before anything shows.",
            "History loads in full before first paint. Paint the last 20 turns, then backfill.",
            "Go ahead.",
            "Done; first paint is under 150 ms on the largest local Session.",
        ],
        0.1,
    )?;
    world.task_run(&recent, "pursue", 0.002, 9, Outcome::Succeeded)?;

    let failed = world.task(
        Some("desktop"),
        "Keep scroll position across Task switches",
        "Switching Tasks resets the conversation to the top.",
    )?;
    let failed = world.checkout(&failed)?;
    world.task_run(&failed, "pursue", 0.3, 18, Outcome::FailedAt(0))?;

    let idle = world.task(
        Some("desktop"),
        "Name Sessions from their first request",
        "Untitled Sessions read as provider ids. Name them from the first request.",
    )?;
    let idle = world.checkout(&idle)?;
    world.session(
        Some(&idle),
        "Session naming",
        &[
            "Sessions show up as 'claude 3f2a91c0'. Can they be named from what I asked?",
            "Yes. The first human turn is already captured; a short title can come from it.",
        ],
        1.5,
    )?;

    let merged = world.task(
        Some("desktop"),
        "Show PR state on Task rows",
        "Rows don't say whether a Task's PR is open, merged or missing.",
    )?;
    let merged = world.checkout(&merged)?;
    world.files(
        Some(&merged),
        &[(
            "src/rows/pr_state.rs",
            "pub enum PrState { Missing, Open, Merged }\n",
        )],
        "Show PR state on Task rows",
    )?;
    world.task_run(&merged, "pursue", 1.0, 14, Outcome::Succeeded)?;
    world.pr(
        &merged,
        21,
        "Show PR state on Task rows",
        "Rows now carry the PR's state.",
        &PASSING,
    )?;
    world.merge(&merged)?;

    for (title, days) in [
        ("Open the dashboard on launch", 6.0),
        ("Remember the selected Wave", 3.0),
    ] {
        let done = world.task(
            Some("desktop"),
            title,
            "Small polish from the last chapter.",
        )?;
        let done = world.checkout(&done)?;
        world.task_run(&done, "pursue", days, 12, Outcome::Succeeded)?;
        world.complete(&done, "Landed", 0.0)?;
    }

    world.wave_conversation(
        "desktop",
        &[
            "What should we pick up next from the backlog?",
            "The backlog has seven unstarted Tasks. \"Rank the Wave backlog\" unblocks \
             most of the others: once order is real, \"Read a brief without opening \
             the Task\" and \"Archive stale backlog ideas\" both build on it.",
            "And the open ones? Anything stuck?",
            "\"Keep scroll position across Task switches\" failed its gate this morning. \
             \"Show PR state on Task rows\" merged but is still open; it can be completed. \
             \"Name Sessions from their first request\" has a conversation and no run yet.",
            "Let's complete the merged one later. Is \"Group open Tasks by where their work stands\" worth doing before ranking?",
            "It's independent. Grouping changes the open list; ranking changes the backlog. \
             Doing ranking first keeps one change in review at a time.",
            "OK, ranking first, then grouping.",
            "Noted: \"Rank the Wave backlog\" next, then \"Group open Tasks by where their work stands\".",
        ],
        0.05,
    )?;
    world.session(
        None,
        "Untracked exploration",
        &[
            "How big is the Wave view's state today?",
            "Three stores: backlog, open Tasks and the conversation.",
        ],
        2.0,
    )?;

    world.task(
        Some("systems"),
        "Compact old Process rows",
        "Process history grows without bound. Compact finished Graphs older than a month.",
    )?;
    let systems = world.task(
        Some("systems"),
        "Retry store writes on busy",
        "A busy store fails the write. Retry with backoff before failing.",
    )?;
    let systems = world.checkout(&systems)?;
    world.task_run(&systems, "pursue", 0.5, 20, Outcome::Succeeded)?;
    Ok(())
}

fn task_review(world: &World) -> Result<()> {
    world.wave("desktop")?;
    world.files(None, &seed_files(), "Lay out the search engine")?;

    let review = world.task(
        Some("desktop"),
        "Rank search results by recency",
        REVIEW_BRIEF,
    )?;
    let review = world.checkout(&review)?;
    world.files(
        Some(&review),
        &[
            (
                "src/search/rank/recency.rs",
                "/// Decay a score by age in days.\npub fn decay(score: f32, age_days: f32) -> f32 {\n    score / (1.0 + age_days / 7.0)\n}\n",
            ),
            (
                "src/search/rank/mod.rs",
                "pub mod bm25;\npub mod recency;\n",
            ),
        ],
        "Add recency decay",
    )?;
    world.files(
        Some(&review),
        &[
            (
                "src/search/query/plan.rs",
                &module("plan", 40, "apply recency after bm25"),
            ),
            (
                "src/search/index/segment/meta.rs",
                &module("meta", 30, "record each document's timestamp"),
            ),
        ],
        "Carry document timestamps into ranking",
    )?;
    world.files(
        Some(&review),
        &[
            (
                "tests/search/recency_test.rs",
                "#[test]\nfn newer_wins_ties() {\n    assert!(decay(1.0, 0.0) > decay(1.0, 30.0));\n}\n",
            ),
            (
                "docs/search/ranking.md",
                "# Ranking\n\nScores are BM25, decayed by age with a one-week half-life.\n",
            ),
        ],
        "Test and document recency ranking",
    )?;
    world.pr(
        &review,
        12,
        "Rank search results by recency",
        "Decays BM25 scores by document age so recent Sessions surface first.\n\n\
         - adds `rank::recency`\n- carries timestamps through segment metadata\n\
         - documents the half-life",
        &PrSurface::Published {
            draft: false,
            checks: &[
                ("lint", "SUCCESS"),
                ("unit", "SUCCESS"),
                ("integration", "FAILURE"),
            ],
            review: "REVIEW_REQUIRED",
            reviewers: &["sam-rivera"],
        },
    )?;
    world.uncommitted(
        &review,
        &[(
            "src/search/rank/recency.rs",
            "/// Decay a score by age in days, with a configurable half-life.\npub fn decay(score: f32, age_days: f32, half_life: f32) -> f32 {\n    score / (1.0 + age_days / half_life)\n}\n",
        )],
    )?;
    world.session(
        Some(&review),
        "Recency ranking",
        &[
            "The integration check failed on the PR. What broke?",
            "`integration` runs the end-to-end search suite. It fails on \
             `old_exact_match_still_first`: recency now outranks an exact title match \
             from last month.",
            "Exact matches should still win. Can we make the half-life configurable and longer by default?",
            "Started locally: `decay` takes a half-life now. It isn't committed yet, so \
             the PR still shows the old version.",
        ],
        0.2,
    )?;

    let draft = world.task(
        Some("desktop"),
        "Highlight matched terms",
        "Results show the whole turn; highlight the matched terms.",
    )?;
    let draft = world.checkout(&draft)?;
    world.files(
        Some(&draft),
        &[(
            "src/search/query/highlight.rs",
            &module("highlight", 25, "wrap matched terms"),
        )],
        "Highlight matched terms",
    )?;
    world.pr(
        &draft,
        13,
        "Highlight matched terms",
        "Wraps matched terms in result snippets. Still a draft.",
        &PrSurface::Published {
            draft: true,
            checks: &[("lint", "SUCCESS"), ("unit", "IN_PROGRESS")],
            review: "",
            reviewers: &[],
        },
    )?;

    let landed = world.task(
        Some("desktop"),
        "Index Codex transcripts",
        "Only Claude transcripts are indexed.",
    )?;
    let landed = world.checkout(&landed)?;
    world.files(
        Some(&landed),
        &[(
            "src/search/index/sources/codex.rs",
            &module("codex", 30, "read Codex rollouts"),
        )],
        "Index Codex transcripts",
    )?;
    world.pr(
        &landed,
        10,
        "Index Codex transcripts",
        "Adds Codex rollouts as an index source.",
        &PASSING,
    )?;
    world.merge(&landed)?;
    world.complete(&landed, "Landed", 0.0)?;

    let merged = world.task(
        Some("desktop"),
        "Rebuild the index on schema change",
        "An index from an older schema returns nothing.",
    )?;
    let merged = world.checkout(&merged)?;
    world.files(
        Some(&merged),
        &[(
            "src/search/index/rebuild.rs",
            &module("rebuild", 30, "rebuild when the schema version differs"),
        )],
        "Rebuild the index on schema change",
    )?;
    world.pr(
        &merged,
        11,
        "Rebuild the index on schema change",
        "Rebuilds an index whose schema version differs.",
        &PASSING,
    )?;
    world.merge(&merged)?;

    let local = world.task(Some("desktop"), "Try a trigram index", "")?;
    let local = world.checkout(&local)?;
    world.files(
        Some(&local),
        &[(
            "src/search/index/trigram.rs",
            &module("trigram", 20, "experiment with trigrams"),
        )],
        "Sketch a trigram index",
    )?;

    let unavailable = world.task(
        Some("desktop"),
        "Page search results",
        "Long result lists load at once; page them.",
    )?;
    let unavailable = world.checkout(&unavailable)?;
    world.files(
        Some(&unavailable),
        &[(
            "src/search/query/page.rs",
            &module("page", 20, "page results"),
        )],
        "Page search results",
    )?;
    world.pr(
        &unavailable,
        14,
        "Page search results",
        "Pages long result lists.",
        &PrSurface::Unavailable,
    )?;
    Ok(())
}

/// Maya's Home is the World; Lee's is `lee/`, with its own `env.json`. Both
/// clone one `origin` that plans through a shared Git ref, and each clone
/// names its person. What they do to each other's work is left to the demo.
fn two_people(world: &World) -> Result<()> {
    world.person("Maya", "maya@example.com")?;
    world.configure("agent: claude\nplanning:\n  provider: git\n  shared: team\n")?;
    world.wave("infra")?;
    world.push_authored("Add the infra Wave")?;
    world.wave_conversation(
        "infra",
        &[
            "Lee is joining this repository from their own Machine.",
            "Planning syncs through the shared Git ref; `lf refresh` brings in each other's Tasks.",
        ],
        0.1,
    )?;
    world.teammate("lee", "Lee", "lee@example.com")?;
    Ok(())
}

fn wave_lifecycle(world: &World) -> Result<()> {
    for wave in ["research", "desktop", "archive"] {
        world.wave(wave)?;
    }
    world.plan(
        "desktop",
        &[("Waves can be added and removed from Desktop", false)],
        &[],
    )?;
    world.task(
        Some("desktop"),
        "Delete an empty Wave",
        "An empty Wave can't be removed from Desktop.",
    )?;
    world.task(
        Some("desktop"),
        "Say what deleting a Wave keeps",
        "Before removing a Wave with work, say what happens to its Tasks and Sessions.",
    )?;
    let open = world.task(
        Some("desktop"),
        "Add a Wave from Desktop",
        "Creating a Wave writes GOAL.md, but the Wave doesn't appear until `lf wave ensure`.",
    )?;
    let open = world.checkout(&open)?;
    world.session(
        Some(&open),
        "Add a Wave",
        &[
            "New Waves from Desktop don't show up.",
            "The sheet writes GOAL.md without a store row. Ensuring the Wave after writing fixes it.",
        ],
        0.5,
    )?;
    world.task_run(&open, "pursue", 0.4, 15, Outcome::Succeeded)?;
    let done = world.task(
        Some("desktop"),
        "List Waves alphabetically",
        "Waves list in creation order.",
    )?;
    let done = world.checkout(&done)?;
    world.task_run(&done, "pursue", 2.0, 10, Outcome::Succeeded)?;
    world.complete(&done, "Landed", 0.0)?;
    world.wave_conversation(
        "desktop",
        &[
            "Which Waves can we remove?",
            "research is empty. archive has only completed work. desktop is active.",
        ],
        0.3,
    )?;

    for (title, days) in [
        ("Retire the old release script", 20.0),
        ("Move fixtures to worlds", 12.0),
    ] {
        let done = world.task(Some("archive"), title, "Finished last chapter.")?;
        let done = world.checkout(&done)?;
        world.task_run(&done, "pursue", days, 15, Outcome::Succeeded)?;
        world.complete(&done, "Landed", days)?;
    }
    Ok(())
}

/// A source file of `lines` numbered functions, so diffs and scrolling have
/// something real to show.
fn module(name: &str, lines: usize, purpose: &str) -> String {
    let mut text = format!("//! {name}: {purpose}.\n\n");
    for line in 0..lines {
        text.push_str(&format!(
            "pub fn {name}_{line}(input: u32) -> u32 {{\n    input.wrapping_mul({}) ^ {line}\n}}\n\n",
            line + 3
        ));
    }
    text
}

/// About 60 files up to five directories deep, one of them ~1,500 lines.
fn seed_files() -> Vec<(String, String)> {
    let mut files = vec![
        (
            "src/search/index/postings.rs".to_string(),
            module("postings", 300, "the inverted index's posting lists"),
        ),
        (
            "src/search/rank/mod.rs".to_string(),
            "pub mod bm25;\n".to_string(),
        ),
        (
            "Cargo.toml".to_string(),
            "[package]\nname = \"search\"\nversion = \"0.1.0\"\nedition = \"2021\"\n".to_string(),
        ),
    ];
    let groups: &[(&str, &[&str])] = &[
        ("src/search", &["lib", "config", "error", "telemetry"]),
        (
            "src/search/index",
            &["mod", "tokenizer", "merge", "writer", "reader"],
        ),
        (
            "src/search/index/segment",
            &["mod", "meta", "store", "compact"],
        ),
        (
            "src/search/index/segment/codec",
            &["varint", "delta", "block", "bitpack", "crc"],
        ),
        ("src/search/index/sources", &["mod", "claude", "files"]),
        (
            "src/search/query",
            &["mod", "parse", "plan", "execute", "filters", "phrase"],
        ),
        ("src/search/rank", &["bm25", "boost", "tiebreak"]),
        ("src/cli", &["main", "args", "output"]),
        (
            "tests/search",
            &["index_test", "query_test", "rank_test", "segment_test"],
        ),
        ("tests/cli", &["args_test", "output_test"]),
    ];
    for (dir, names) in groups {
        for name in *names {
            files.push((
                format!("{dir}/{name}.rs"),
                module(name, 12, &format!("{dir} {name}")),
            ));
        }
    }
    for (name, title) in [
        ("README", "Search"),
        ("architecture", "Architecture"),
        ("indexing", "Indexing"),
        ("querying", "Querying"),
        ("ranking", "Ranking"),
        ("segments", "Segments"),
        ("codecs", "Codecs"),
        ("operations", "Operations"),
        ("benchmarks", "Benchmarks"),
        ("roadmap", "Roadmap"),
    ] {
        files.push((
            format!("docs/search/{name}.md"),
            format!("# {title}\n\nNotes on {name} for the search engine.\n"),
        ));
    }
    files
}

const REVIEW_BRIEF: &str = r#"# Rank search results by recency

Search ranks purely by BM25 today. A phrase that appears in a Session from
last spring outranks the same phrase from this morning, and people end up
scrolling past old work to find what they just did.

## Outcome

Recent Sessions surface first when relevance is close, without burying an
exact match from last month.

## Who notices

- Someone searching for "the migration we discussed" this week finds this
  week's Session at the top.
- Someone searching an exact Session title still gets that Session first,
  however old it is.

## Approach

1. Carry each document's timestamp into segment metadata
   (`src/search/index/segment/meta.rs`).
2. Add `rank::recency::decay(score, age_days)`, a hyperbolic decay with a
   one-week half-life.
3. Apply the decay after BM25 in the query plan
   (`src/search/query/plan.rs`), before tie-breaking.

```rust
pub fn decay(score: f32, age_days: f32) -> f32 {
    score / (1.0 + age_days / 7.0)
}
```

### Why hyperbolic

Exponential decay makes month-old work effectively invisible. Hyperbolic
decay halves a score after a week and keeps a long tail, so an exact
match still competes.

| age    | exponential | hyperbolic |
|--------|-------------|------------|
| 1 day  | 0.91        | 0.88       |
| 1 week | 0.50        | 0.50       |
| 1 month| 0.05        | 0.19       |

### What stays the same

- Tokenization and posting lists.
- Phrase queries: recency applies to the phrase's score as a whole.
- The CLI's output format.

## Edge cases

- **Missing timestamps.** Documents indexed before this change have none.
  Treat them as age zero until the next rebuild, rather than as ancient.
- **Clock skew.** A timestamp in the future clamps to age zero.
- **Exact title match.** Title matches get a boost large enough to survive
  a month of decay; `tests/search/rank_test.rs` pins the crossover.

## Checks

- `cargo test -p search` passes, including the new
  `tests/search/recency_test.rs`.
- The integration suite's `old_exact_match_still_first` passes.
- Ranking docs in `docs/search/ranking.md` describe the half-life.

## Open questions

- Should the half-life be configurable per repository? A research repo may
  want months; a busy product repo, days.
- Do we decay Graph output the same as conversation turns?

## Out of scope

- Personalized ranking.
- Re-ranking with a model.
- Any change to how results are displayed; see
  [Highlight matched terms](https://example.com/tasks/highlight).

## Notes from review

> The decay is applied after BM25 but before the title boost, which means
> the boost has to be tuned against decayed scores. Consider applying the
> boost first.

That ordering is deliberate: the boost is a floor, not a multiplier. The
test in `rank_test.rs` names the crossover explicitly so a future change to
either constant fails loudly.

## Rollout

1. Land behind no flag; the change is small and reversible.
2. Rebuild indexes lazily: the first query after upgrade triggers it.
3. Watch query latency in telemetry for a week.

## Follow-ups

- Per-repository half-life if the open question resolves that way.
- Surface "why this result" on hover, showing BM25, decay and boost.
"#;
