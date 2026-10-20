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
        name: "density",
        rung: Rung::Artifacts,
        demos: "sidebar density: 150 Tasks across three Waves, 20 started, orphan Sessions",
        build: density,
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
        "search",
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
        "desktop",
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
    world.complete(&shipped, "Landed")?;

    let running = world.task(
        "desktop",
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
        "systems",
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
        "systems",
        "Copy the store for unreleased builds",
        "A build with drafts works on a fresh copy of the real store.",
    )?;
    world.task(
        "quality",
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
            let task = world.task(wave, &title, "One of many Tasks in a dense Wave.")?;
            let global = wave_index * 50 + index;
            if global < 140 && global % 7 == 0 {
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
