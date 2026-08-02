# ADR 0001: Rust/Tauri port of chartr

- **Status:** Accepted
- **Date:** 2026-08-02

## Context

stellr is a fresh Rust/Tauri port and generalization of
[chartr](https://github.com/rengwu/chartr). The port keeps chartr as a reference
implementation while giving the issue graph, GitHub integration, server, and
application shell explicit boundaries. The standing architecture is described
in the [port design](../specs/2026-07-29-stellr-port-design.md), and the M1 slice
is detailed in the [implementation plan](../plans/2026-07-29-m1-chart.md).

M1 must provide one binary that can synchronize GitHub issues and serve the
star-map in a normal browser or an IDE pane. Later milestones add the native
Tauri shell and terminal multiplexer without replacing that browser-capable
transport.

## Decision

1. **Use a Cargo workspace with four M1 crates:** `stellr-core` owns the pure
   issue-graph domain, `stellr-github` implements GitHub access,
   `stellr-server` owns the axum transport, and `stellr-app` assembles the
   executable. This keeps domain policy independent from I/O and gives each
   boundary focused tests.
2. **Embed axum instead of routing application traffic through pure Tauri IPC.**
   HTTP plus dedicated WebSockets preserve terminal throughput, avoid
   head-of-line blocking between terminal and map updates, and let the same UI
   run in VS Code or another browser pane.
3. **Ship GitHub Issues as the sole provider behind the `Provider` trait.** The
   trait keeps provider-specific I/O out of the domain without promising extra
   providers before there is a concrete need.
4. **Separate GitHub credentials from the local browser session.** M1 resolves
   GitHub access from `GITHUB_TOKEN` or `gh auth token`. `stellr serve`
   generates a per-run token, accepts it through the printed URL, and exchanges
   it for a strict HTTP-only session cookie; bearer authentication remains
   available for clients.
5. **Defer GitHub device-flow authentication to M2.** Device flow needs native
   UI and secure-storage surfaces that are outside the headless M1 slice; M1
   instead requires an existing `gh` login or `GITHUB_TOKEN`.
6. **Poll every 30 seconds in M1.** A fixed interval is predictable and enough
   for the first read-only map; focus-aware backoff belongs with the Tauri
   lifecycle in M2.

## Consequences

- The web bundle must be built before compiling `stellr-app`, because the
  server embeds the generated assets.
- The headless and future desktop modes share one server-authoritative model
  and transport rather than growing separate application paths.
- M1 remains read-only and intentionally omits device flow and focus-aware
  polling; those omissions are scheduled decisions, not accidental gaps.
- Adding another provider requires a concrete implementation of the existing
  provider boundary rather than changes to the core issue model.
