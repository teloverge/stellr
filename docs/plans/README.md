# Implementation plans

One plan per milestone, written from the approved spec
(`../specs/2026-07-29-stellr-port-design.md`) when the milestone starts —
not before. Planned sequence:

| Milestone | Plan | Status |
| --- | --- | --- |
| M1 — the chart | [`2026-07-29-m1-chart.md`](2026-07-29-m1-chart.md) | ready |
| M2 — the shell (Tauri window, theme, installers) | written when M1 lands | — |
| M3 — the multiplexer (terminals, sessions) | written when M2 lands | — |
| M4 — the senses (detection, notifications, polish) | written when M3 lands | — |

## Execution-mode convention

Every plan in this directory starts with an **execution gate**: the
implementing agent must ask the operator to choose, at the start of each
milestone's implementation (and again on any fresh session resuming it),
between:

1. **Subagent-Driven** — superpowers:subagent-driven-development; fresh
   subagent per task, review between tasks.
2. **Inline** — superpowers:executing-plans; tasks in the main session with
   checkpoints.

The choice never carries over between milestones or sessions. New milestone
plans must copy the gate block from the top of the M1 plan.
