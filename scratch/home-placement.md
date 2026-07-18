# Authoritative Home placement and one shared resident

## Product contract

Every durable `WorkRef` has exactly one `HomeId` before its first Run. A Run
cannot choose another Home at reservation time. Moving Work is an explicit
placement write and is refused while that Work has a live Run.

One Home keeper process serves every Wave placed on that Home. Wave listeners
remain the per-Wave chat/journal boundary, but they are tasks inside the Home
keeper rather than detached control processes. Provider bodies remain Launches;
they are not residents.

`lf start [WAVE...]` is the public orchestration verb:

1. read each Wave Work's durable placement;
2. group Waves by Home;
3. ensure one keeper per local Home;
4. ask that keeper to serve each local Wave;
5. recurse to remote Homes through remote-native SSH.

Remote groups recurse only through `lf ssh <HomeId> --remote-native -- lf start
...`. First contact uses a raw SSH route to read the remote's stable local
`HomeId`; later calls resolve the mutable route by id.
The remote validates the expected HomeId before changing state.
The internal hop carries each exact WaveId with its human name; the target
refuses a name/id collision rather than minting a second Work identity.

## Data model

```rust
struct Placement {
    work: WorkRef,
    home_id: HomeId,
    placed_at: OffsetDateTime,
}
```

`work_placements` uses the same one-of-three typed foreign-key shape as Epoch:
`wave_id | project_id | task_id`, one non-null, unique per Work. New Project
and Task Work inherits its parent placement once at creation. A later explicit
move changes only that Work.

`Home { id, route, ... }` remains the single Home record. `HomeId` is identity;
`route` is mutable observation. The existing local Home row created by the
durable-input migration is the stable migration source. Remote first contact
reads that row from the remote store and records its SSH route locally.

## Public APIs

```rust
Store::placement(&WorkRef) -> Placement
Store::place_work(&WorkRef, &HomeId) -> Placement
Store::home_by_id(&HomeId) -> Option<Home>
Store::local_home() -> Home
Store::observe_home(&HomeId, route: &str) -> Home
Store::reserve_run(&WorkRef, RunTrigger) -> (Run, RunLease)
```

Removing `home_id` from `reserve_run` is the important fence: callers cannot
bypass placement. Reservation also requires placement to name the local Home,
so local containment can never be recorded as a remote Run. Legacy
Session-to-Run bridge writes use the same invariant rather than selecting the
first Home row.

The public mutation is `lf work place <kind> <id> <HomeId>`. `lf start --json`
returns existing `WaveSnapshot` rows; the keeper's loopback start response is
not a second public lifecycle model.

## Residency

The Home keeper exposes loopback health/start control and writes a discovery
pointer keyed by `HomeId`. Starting twice returns the existing served Wave.
Stopping one Wave shuts down only its listener/Run; the keeper and unrelated
Waves remain live.

## Done when

- every Wave, Project, and Task Work row has one placement;
- Run reservation fails without placement and always records its placement;
- placement cannot move while a Run is live;
- one keeper process starts two Wave listeners and stopping one leaves the
  other and the keeper live;
- `lf start` groups by Home and remote groups use remote-native `lf ssh`;
- `lf ssh <HomeId>` resolves the route and validates remote identity;
- status/DTO/docs expose HomeId without retaining address text as identity;
- Rust format, clippy, tests, Python tests, Swift tests, and prompt goldens pass.
