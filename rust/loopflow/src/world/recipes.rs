//! Named worlds. Each recipe says which rung of the ladder it climbs to and
//! what it is the right venue to demo.

use anyhow::Result;

use super::{Threshold, World};

/// Rows: store records through production writers. Artifacts: also files a
/// reader opens, such as provider transcripts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum Rung {
    Rows,
    Artifacts,
}

impl Rung {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Rows => "rows",
            Self::Artifacts => "artifacts",
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
        demos: "Wave → Task → Session → Flow click-through on one small piece of work",
        build: one_task,
    },
    Recipe {
        name: "portfolio",
        rung: Rung::Artifacts,
        demos: "dashboards and Portfolio: several Waves with KRs and Tasks in every state",
        build: portfolio,
    },
    Recipe {
        name: "lifecycle",
        rung: Rung::Artifacts,
        demos: "Task history: paused, abandoned, canceled, standalone and weeks of completions",
        build: lifecycle,
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
    world.flow(
        Some(&task),
        "implement",
        &["implement", "gate"],
        0.1,
        12,
        "succeeded",
    )?;
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
    world.flow(
        Some(&shipped),
        "implement",
        &["implement", "gate"],
        3.5,
        20,
        "succeeded",
    )?;
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
    world.flow(
        Some(&running),
        "implement",
        &["implement"],
        0.05,
        15,
        "succeeded",
    )?;

    let failed = world.task(
        Some("systems"),
        "Record a reader floor per migration",
        "Additive migrations should not lock out older lf.",
    )?;
    let failed = world.checkout(&failed)?;
    world.flow(
        Some(&failed),
        "implement",
        &["implement", "gate"],
        1.0,
        25,
        "failed",
    )?;

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
        world.flow(
            Some(&task),
            "implement",
            &["implement", "gate"],
            days + 0.1,
            15,
            "succeeded",
        )?;
        world.complete(&task, "Landed", days)?;
        if removed {
            world.remove_checkout(&task)?;
        }
    }
    let quarter = world.task(wave, "Shipped last quarter", "Sign the macOS build in CI.")?;
    world.complete(&quarter, "Landed", 40.0)?;

    let paused = world.task(
        wave,
        "Paused mid-flight",
        "Move the changelog into the release workflow.",
    )?;
    let paused = world.checkout(&paused)?;
    world.session(
        Some(&paused),
        "Changelog workflow",
        &[
            "Generate the changelog in the release workflow.",
            "Started; waiting on the new tag format before going further.",
        ],
        2.0,
    )?;
    world.flow(
        Some(&paused),
        "implement",
        &["implement"],
        1.8,
        20,
        "succeeded",
    )?;
    world.pause(&paused)?;

    let abandoned = world.task(
        wave,
        "Abandoned attempt",
        "Publish nightly builds to a second channel.",
    )?;
    let abandoned = world.checkout(&abandoned)?;
    world.flow(
        Some(&abandoned),
        "implement",
        &["implement", "gate"],
        3.0,
        25,
        "failed",
    )?;
    world.abandon(&abandoned)?;

    let canceled = world.task(
        wave,
        "Canceled upstream",
        "Mirror releases to a second registry.",
    )?;
    let canceled = world.checkout(&canceled)?;
    world.flow(
        Some(&canceled),
        "implement",
        &["implement"],
        2.5,
        15,
        "succeeded",
    )?;
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
    world.flow(
        Some(&running),
        "implement",
        &["implement"],
        0.05,
        15,
        "succeeded",
    )?;

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
