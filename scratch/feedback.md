# Feedback at a Work Boundary

## Product contract

A flow step declares `feedback: true` when it stays current while another
actor responds. The actor may be the User or the immediate parent Run.
Feedback does not imply approval, disposition, or formal review.

```yaml
- step:
    name: design
    feedback: true
```

The runtime derives one current Feedback boundary from Work, flow position,
Launch, Basis, and attention route. There is no Feedback row or Feedback id.

```text
lf queue
lf work feedback task <id>
lf work continue task <id>
lf work escalate task <id>
```

`lf work feedback` presents the Feedback's recorded Launch route. It does not
implement a second conversation protocol or translate terminal input into
Steers. For Task and Project Work this currently opens the recorded tmux
provider session through `lf launch present`.

`lf work continue` ends the Feedback boundary and advances the flow. With no
feedback supplied it is functionally a skip; after conversation it means the
User or parent is done responding.

## Exit policies

- default: presentation exit leaves Feedback open;
- `--continue-on-success`: exit status zero continues Feedback;
- `--continue-on-exit`: every presentation exit continues Feedback, and a
  detached exact-Launch/Basis guard also continues if the wrapper is killed.

All continuation is fenced by Work, Launch, and Basis. A replacement Feedback
turns a late exit into a harmless no-op.

## Done when

- no flow, CLI, DTO, or Concerto API calls this boundary Review;
- `feedback: true` is flow-local and does not leak from skill frontmatter;
- `lf work feedback` runs the recorded Launch presentation route;
- exit-policy tests prove leave-open, success-only, any-exit, and stale fencing;
- Rust formatting, clippy, affected tests, Swift tests, and boundary checks pass.
