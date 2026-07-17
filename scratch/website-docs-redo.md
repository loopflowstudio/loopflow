# Website + Docs Redo

Design doc for the total rewrite of `website/` and `docs/`. Study phase complete
(2026-07-16); this records what we learned and the target structure.

## What the study found

### The product today (source of truth: root README.md + rust source)

- One binary. `lf wave <name>` boots a self-contained resident (listener +
  resident) for one Wave. No global service; `lfd` survives only as webhook
  ingress + liveness. v0.11.0 deleted the distributed system on purpose
  (`release/v0.11.0/NOTES.md`: "the release where loopflow stops being a
  distributed system").
- **Agent API**: agents drive other agents entirely through `lf` verbs — no
  SDK. `lf task run/steer/follow-up/receipt/decide/acknowledge`, `lf project
  ...`, `lf radio pub/sub` (agents-only bus; publish is an INSERT, no broker),
  `lf memory add --receipt`, `--json` on every read surface. The contract is
  taught to every launched agent via the bundled `LOOPFLOW.md`
  (`rust/loopflow/src/engine/builtins/LOOPFLOW.md`).
- **Fleet monitoring/steering**: `lf ls / status / roadmap / runs / execs /
  trace / context / top / usage / ci / doctor`, `lf chat --steer`, task/project
  steer+receipt, handoffs. The Mac app is the cockpit — a pure client over
  `lf ... --json`, no second database.
- **Decentralization = no server, by subtraction.** Per-machine SQLite
  (`~/.lf/loopflow.db`) + append-only journals are local truth; Linear and
  GitHub are the only shared truth; cross-machine reach is plain SSH via the
  Home model (`ssh://owner@host` in GOAL.md frontmatter) with process-lifetime
  credential forwarding (`lf ssh`) so remote hosts stay stateless. A company
  deploys it by reusing its Linear + GitHub + Doppler + OS-keychain boundaries
  and adding nothing central.

### Evolution (for the story page)

Maestro one-shot CLI (Dec 2025) → waves + lfd daemon (Jan–Feb) → agent-API +
chord/VSM peak complexity (Feb–Mar) → "pick a side": orchestration layer above
the vendors, not a chat client (Apr–Jun, v0.9.11) → minds: reactive wave server
replaces the goal loop (Jul, v0.10.0) → the server deletion (v0.11.0) → Homes,
profiles, fleet ledger (v0.11.2+, now). Chronology lives in `release/v*/NOTES.md`.

### Current publishing state (the mess to simplify)

Three sources of truth telling three stories:

1. Root `README.md` — freshest, canonical.
2. `docs/*.md` — served live by the website's FastHTML markdown renderer
   (`website/main.py` falls back to repo-root `docs/`), PLUS a stale parallel
   Jekyll config (`docs/_config.yml`).
3. `website/content.yaml` + `main.py` — homepage still pitches the Maestro-era
   story: "Download for Mac", orchestra metaphor, 6 capabilities / 4 code
   cards / 2 product cards, hero poster `/static/loopflow-main.png` which
   **does not exist** (assets still named `maestro-*` / `concerto-*`).

Plus: `llms.txt` hard-coded in `main.py` describing the old concept set; e2e
tests (`website/tests/e2e/test_homepage.py`) pin the old narrative verbatim
(exact tagline, section counts, the 404 poster path); `/team` page reads
"Founder & CEO … actively looking for co-founders".

Stale-doc highlights: `getting-started.md` calls remote execution "future
work" (Homes shipped); `architecture.md` never mentions Homes, `lf ssh`, or
reduced `lfd`, and isn't even in the site nav; `index.md` teaches tmux as the
steering surface (it's read-only inspection now).

## Target structure

### Principles

- **One markdown tree is the product's written truth.** The site renders it;
  nothing restates it. `README.md` becomes a short pointer into it (or is
  generated from the same pages).
- **The site carries the story; docs carry the system.** Homepage = the
  narrative (what this is, why no server, the dogfooding arc) in a few
  screens. Everything factual lives in docs pages the homepage links into.
- **Tests pin structure, not copy.** e2e asserts pages render, nav works,
  assets resolve — never exact taglines or section counts, so copy edits
  don't require test edits.
- **`llms.txt` is generated from the docs tree**, not hard-coded.

### Docs tree (~8 pages, each mapped to a shift or a job)

| Page | Job | Salvage |
|---|---|---|
| `index.md` | What loopflow is now, in one screen; routes by reader (human bootstrapping / agent integrating / operator monitoring) | rewrite |
| `getting-started.md` | Human bootstrap: install, `lf init`, auth, first wave | heavy edit |
| `agent-api.md` | **NEW.** `lf` as the API agents call: nouns, delegation, steering protocol, radio bus, receipts, memory | mine `lf.md` + LOOPFLOW.md |
| `fleet.md` | **NEW.** Monitoring & steering many agents: the read surfaces, steer verbs, handoffs, the Mac cockpit | mine `lf.md` ledger section |
| `architecture.md` | No server by subtraction: store/journal, Linear+GitHub as shared truth, Homes + `lf ssh`, what a company deploys | extend existing |
| `waves.md` | Wave/Project/Task model + authoring GOAL/MEMORY (merge `wave-authoring.md` in) | merge + edit |
| `lf.md` | Flat command reference | keep, restructure |
| `config.md`, `ops.md`, `troubleshooting.md` | Reference | keep, light touch |

Delete: `docs/_config.yml` (Jekyll path), `docs/stubs/`, `concerto-*`
screenshots (re-shoot from the current Mac app or drop).

### Website

Keep the FastHTML app (it already renders repo docs live; deploy works), but
shrink it:

- Homepage: new narrative from the four shifts. Tagline direction: waves /
  persistent agents / no server. CTA: the curl install + "read the docs";
  Mac app download second.
- `/team` → `/story` (or fold into homepage): the dogfooding narrative —
  built by one person who runs it all day, telling the story publicly.
- Kill dead hero video slot or point it at a real capture.
- Fix/rename all `maestro-*`/`concerto-*` assets; create the referenced
  hero image from a current screenshot.
- Regenerate `llms.txt` from docs.
- Rewrite e2e per the testing principle above.

## Decisions (resolved with Jack, 2026-07-16)

1. Homepage is **story-first**: hero tagline states the posture, a "Built on
   itself" narrative section leads, product pillars follow.
2. The story is told **third-person, product-centric** (`/story`, replacing
   `/team`); Jack can add a first-person letter later.
3. **Waitlist deleted** (route, db.py, psycopg2/resend deps, dev.py `db`
   subcommand).

## What shipped

- Docs: rewritten `index.md`; new `agent-api.md` and `fleet.md`;
  `architecture.md` extended (No server, Homes, lfd); `wave-authoring.md`
  merged into `waves.md`; `getting-started.md` Go Remote rewritten around
  Homes; `lf.md` got an audience map; Jekyll `_config.yml`, `stubs/`, and
  `concerto-*` screenshots deleted.
- Website: story-first homepage from new `content.yaml` schema; `/story`
  page rendered from `website/story.md`; `/team|/loopflow|/maestro`
  redirect; `/download` covers CLI + Mac app; `llms.txt` rebuilt (doc list
  generated from `DOCS_NAV`); dead video/vocab/product-card code deleted;
  maestro/concerto-era static assets deleted.
- Renderer: fixed an infinite loop on prose lines containing `|` (newly
  exposed because tests now render every nav page).
- Tests: e2e now pin structure, not copy — every `DOCS_NAV` page must render
  with an h1, homepage images must resolve, redirects and `llms.txt` pinned;
  copy edits in `content.yaml`/docs no longer break tests. 60 passed.

## Agent-first docs (added 2026-07-17)

99% of readers are agents. Following Vercel/Stripe/Cloudflare practice, the
site serves: `/docs/<slug>.md` raw markdown (frontmatter: title,
canonical_url, last_updated), `Accept: text/markdown` negotiation, markdown
404s with nearest-match suggestions, spec-shaped `/llms.txt`,
`/llms-full.txt`, `/sitemap.xml`, and a View-as-Markdown link per page.
Writing rules adopted: sections stand alone, exact strings verbatim, nothing
conveyed only visually, examples carry the weight.

**No MCP, ever** — Jack's posture is CLI-only. The distribution channel for
teaching outside agents is a published skill: `skills/loopflow/SKILL.md`
(the LOOPFLOW.md contract reframed for agents not launched by lf, installable
via `npx skills add loopflowstudio/loopflow`). Keep it aligned with
`rust/loopflow/src/engine/builtins/LOOPFLOW.md`.

## Deferred

- Fresh screenshots/demo capture of the current Mac app (all prior assets
  were Maestro/Concerto-era and were deleted rather than shown stale).
- The hero has no visual; add one when a current capture exists.
- Verify `npx skills add loopflowstudio/loopflow` resolves the new
  `skills/loopflow/SKILL.md` once it lands on the public default branch.

## Not doing

- No new publishing stack; no MDX/static-site migration.
- No rewrite of `rust/loopflow/src/**/README.md` (maintainer docs).
