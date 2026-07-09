# Loopflow API

The loopflow API is the product contract for goal-authored computation. Waves,
projects, tasks, agents, runs, chat, delegation, termination, recovery, and
evidence all share one coherent model across CLI, Mac, iOS, prompts, and worker
processes.

## KRs

- Wave, project, task, run, agent, chat, and evidence APIs are documented as
  one product model.
- CLI, Mac, iOS, and agent prompts expose the same wave/project/task model
  without parallel concepts.
- A new wave can be created, steered, delegated, inspected, and closed through
  the shared API contract.
- Task loops can drive a red PR green unattended, then stop with an actionable
  non-convergence record when they cannot.
- Wave loops can spawn project loops, project loops can spawn task loops, and
  caps remain enforced.
