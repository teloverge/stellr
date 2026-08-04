# M2 Native Shell Design

**Issues:** #57 — Complete Stellr v1; #18 — M2: the shell

**Status:** Approved by the project owner on 2026-08-02

## Goal

Turn the shipped browser-hosted M1 chart into a daily-driven Tauri 2 desktop
application without creating a second application model or weakening
`stellr serve`. M2 adds the native window lifecycle, secure in-app GitHub
authentication, protocol and single-instance routing, a complete light/dark
visual system, and installable artifacts for the first supported architecture
matrix.

## Scope

M2 delivers:

- a Tauri 2 host for the existing embedded axum application;
- default desktop launch plus `serve` and `open` CLI modes;
- one running application instance with argument and deep-link forwarding;
- OS-keychain credential persistence and GitHub OAuth device flow;
- lifecycle-aware polling based on native window focus;
- persisted window and theme state;
- a system tray with Open and Quit;
- native external-link and local-directory selection;
- the Polar Observatory light/dark visual system;
- Windows x64, universal macOS, and Linux x86_64 packages;
- native CI, development-bundle, and fail-closed tagged-release gates.

M2 does not add PTYs, agent launches, issue claim/release writes, terminal
WebSockets, session-state detection, notifications, the global summon shortcut,
the command palette, milestone hulls, minimap, search/filter, or new providers.
Those remain sequenced into M3 and M4.

## Existing Foundation

The M1 crates, server model, browser authentication, GitHub synchronization,
offline cache, space lifecycle, browser route, issue detail, deterministic
star-map renderer, dependency paths, and compact subissue workflows remain the
foundation. M2 extends those seams instead of replacing them.

The accepted port ADR continues to govern the architecture:

- axum, not pure Tauri IPC, carries application traffic;
- desktop and browser hosts share one server-authoritative model;
- GitHub Issues remains the only provider;
- the provider credential and local browser session credential remain distinct;
- native concerns alone cross Tauri IPC.

## Runtime Architecture

### Shared application runtime

Extract application assembly from the current `serve` command into one reusable
runtime. It owns:

- loopback listener creation;
- per-run local session-token generation;
- space-store and cache loading;
- application state and provider lifecycle;
- axum router construction;
- polling lifecycle;
- graceful server shutdown.

The runtime returns its bound address, authenticated cockpit URL, state handle,
and shutdown handle to its host. It accepts `127.0.0.1:0` for desktop mode and
the explicit `serve --addr` value for browser mode.

### Browser host

`stellr serve` preserves its current public behavior: resolve an existing
credential, bind the requested address, print one authenticated cockpit URL,
and remain attached to the server process. Existing command and process tests
remain unchanged and green.

### Desktop host

Bare `stellr` starts Tauri, starts the shared runtime on a random loopback port,
and loads its authenticated URL in one webview. The native window does not load
the bundled SPA through a separate Tauri asset protocol; axum remains the only
application host.

The desktop webview uses the same one-time query-token exchange and strict
HTTP-only session cookie as browser mode. Tauri IPC is not an alternate model
transport.

### Native bridge

The native bridge is deliberately narrow. It exposes only:

- open an external URL;
- choose a local repository directory;
- read and update the theme preference;
- report native focus changes to the polling lifecycle;
- focus and route the main window after a second launch or protocol link.

All space mutations, model snapshots, and refresh actions continue through the
existing HTTP and control-WebSocket interfaces.

## CLI and Navigation

The public command surface becomes:

```text
stellr                    open or focus the desktop app without requiring a repository
stellr serve [options]    run the browser/IDE host
stellr open <path|url>    open or focus a path, owner/repo, GitHub URL, or stellr link
stellr --version          print the application version
```

Bare launch restores persisted spaces when available and otherwise exposes the
existing empty repository-selection shell. `open` accepts:

- a local repository path;
- an `owner/repo` slug;
- a supported GitHub repository or issue URL;
- a canonical `stellr://space?...&issue=N` link.

All targets normalize into one route request before they reach the window.
Invalid targets return a visible error without replacing the current valid
space or selection.

The single-instance plugin is registered first. A second invocation forwards
its arguments and working directory, exits, and causes the first instance to
show, focus, and apply the normalized route. A protocol link uses the same
normalizer and routing path. Browser hash routes remain supported and are not
replaced by the OS protocol.

## GitHub Authentication

### Credential precedence

Credential resolution is deterministic:

1. nonblank `GITHUB_TOKEN`, for automation and explicit operator override;
2. a successful `gh auth token` result;
3. the token stored in the OS keychain for Stellr;
4. unauthenticated application state, which enables device flow in desktop mode.

`serve` remains noninteractive and returns the existing helpful error when the
first three sources are unavailable.

The keychain service name is `stellr.github`; the account is `default`. A
keychain implementation is injected behind a small credential-store interface
so public resolution behavior can be tested without reading a developer's real
credential store.

### Device flow

The registered OAuth App public client ID is `Ov23liWXBEZ0ysYu2MxE`. Device
flow requests the `repo` scope selected by the approved product design. A client
secret is neither required nor shipped.

The Rust-side device-flow client:

1. requests a device code;
2. publishes only the user code, verification URI, expiry, and polling state;
3. polls no faster than GitHub's returned interval;
4. adds five seconds after each `slow_down` response;
5. stops on success, cancellation, denial, expiry, or unrecoverable error;
6. persists a successful token to the OS keychain;
7. constructs the GitHub provider and starts synchronization for the current
   application state without restarting the process.

The access token never enters the web model, WebSocket, browser storage, URL,
log, error text, or context file. If keychain persistence fails, the provider
remains usable for the current run and the UI states clearly that sign-in will
not survive restart.

### Authentication UI

When no credential is available, the desktop window opens normally and shows a
focused sign-in panel within the existing shell. It contains:

- the eight-character user code;
- GitHub's verification URI;
- Copy Code and Open GitHub actions;
- remaining lifetime and current polling state;
- Cancel and Retry actions;
- specific denial, expiry, network, and persistence errors.

Cached spaces remain visible and navigable behind the authentication state when
available. Provider refresh waits until authentication succeeds.

## Polling Lifecycle

Replace the fixed poll duration with an interval controller. Its public state is
Focused or Background:

- Focused schedules approximately every 30 seconds.
- Background schedules approximately every five minutes.
- Manual refresh wakes immediately in either state.
- A transition recalculates the next deadline; it does not wait for the old
  interval to expire.

Desktop window focus drives the controller. `serve` supplies Focused
continuously so its established 30-second behavior remains unchanged.

## Window and Tray Lifecycle

The main window uses native OS decorations and controls. M2 does not implement a
custom title bar.

Persist and restore:

- window size and position;
- maximized state;
- theme preference;
- current browser route through the existing route model.

An off-screen restored position is rejected by the window-state plugin's native
monitor handling.

Closing the main window exits Stellr. The tray menu contains Open and Quit:

- Open shows and focuses the existing main window.
- Quit exits the application cleanly.

M2 does not hide on close. That behavior may be reconsidered in M3 when
persistent terminal sessions create a concrete reason to remain in the
background.

## Visual System: Polar Observatory

M2 replaces the placeholder shell styling with the approved Polar Observatory
system.

### Character

- cool-neutral, low-chroma surfaces;
- crisp cobalt as the single interactive emphasis;
- restrained native chrome suitable for all-day use;
- the pure-black star map remains the visually dominant canvas;
- data-visualization status colors remain behind the renderer theme seam.

### Modes

System is the first-launch theme choice. The operator may explicitly select
Light, Dark, or System; the preference persists locally.

Light mode uses porcelain and cool-gray surfaces with dark ink text. Dark mode
uses graphite and deep slate surfaces with near-white text. Both modes keep
border, muted, focus, destructive, success, warning, and cobalt emphasis tokens
at accessible contrast.

All chrome colors are CSS custom-property tokens. Components do not contain raw
color literals. The existing renderer is not edited to consume shell colors.

### Components

Use a small vendored primitive set for buttons, inputs, dialogs, dropdown menus,
tooltips, tabs, and toasts. Components own interaction and accessibility;
feature views own domain behavior. Phosphor icons replace ad-hoc symbol glyphs.
No CDN assets or runtime font/icon fetches are introduced.

## Error and Security Model

- Native-server bind or startup failure shows a native error dialog and exits;
  the app never opens a broken webview.
- Provider, rate-limit, and offline failures preserve cached navigation and
  remain visible through existing stale/error state.
- Invalid second-instance arguments or deep links surface in the existing
  window and do not discard its current route.
- Device-flow denial, expiry, and cancellation are distinct recoverable states.
- Keychain errors never include token content.
- The server binds to loopback by default and retains constant-time local-token
  comparison and strict cookie behavior.
- External links are limited to parsed `https` URLs and the approved GitHub
  verification URI.
- Local path selection returns a path only after explicit user action.
- The CSP and Tauri capability configuration grant only the native bridge
  operations used by M2.

## Testing Strategy

Tests observe supported external behavior rather than private collaborators.

### Primary seam

The application process is the primary acceptance seam. A deterministic native
scenario launches the real Stellr binary with a controlled GitHub service and a
test credential store, completes device flow through the public UI/HTTP
contract, opens a space, receives a synchronized model, exercises focus-aware
polling, closes, relaunches, and restores window, theme, and route state.

A second real process proves single-instance forwarding and focus/reroute
behavior. The same shared runtime is exercised through `stellr serve` to prove
the browser host remains compatible.

### Focused seams

- CLI tests cover default desktop launch, target normalization, positive issue
  numbers, and canonical protocol links.
- Authentication integration tests cover credential precedence, keychain
  success/failure, device-flow success, pending, slow-down, denial, expiry,
  cancellation, and network errors.
- Server tests cover unauthenticated state, provider activation, protected
  routes, lifecycle-controlled polling, and unchanged browser auth.
- Frontend tests cover sign-in states, copy/open actions, theme resolution and
  persistence, route application, and recoverable native errors.
- Tauri tests cover native command behavior with mock runtime handles where
  platform APIs cannot run in a unit process.
- Native smoke tests cover real window launch, tray Open/Quit, Close-exits,
  single instance, protocol links, and application restoration.

Every existing Rust and frontend test remains green.

## Packaging and Release

The first release matrix is:

- Windows 11 x64: NSIS installer;
- macOS: universal Intel and Apple Silicon application in a DMG;
- Linux x86_64: AppImage and Debian package.

Windows ARM64 and Linux ARM64 are deferred until demand exists.

Local and pull-request workflows may produce unsigned development bundles, but
their filenames and workflow summaries must identify them as unsigned and not
for release. Official tagged releases fail before publication unless Windows
and macOS signing credentials are configured. No unsigned artifact may be
silently attached to an official release.

The Tauri release action builds on native platform runners. CI gates:

- frontend install, test, check, and production build;
- Rust formatting, warnings-denied Clippy, workspace tests, and build;
- Tauri compilation on every supported host;
- package construction and platform-specific inspection;
- Windows install, launch, uninstall, and WebView2 smoke;
- both architecture slices in the universal macOS binary plus signed launch;
- Linux AppImage and Debian install/launch smoke.

Release notes remain append-only and newest-first. M2 work is added only under
Unreleased until a version ships; shipping creates a new version section rather
than rewriting an older one.

## Acceptance Criteria

M2 is complete when:

- bare `stellr` opens the native shell without requiring a repository;
- `serve` retains its documented browser behavior;
- `open` and `stellr://` route into one running instance;
- device flow signs in without exposing the access token to the webview;
- a successful token survives restart through the OS keychain;
- focus changes alter polling cadence while manual refresh remains immediate;
- window, theme, space, and issue state restore;
- Polar Observatory passes light/dark accessibility and headed review;
- tray Open/Quit and Close-exits behave as approved;
- development bundles build on the three native platforms;
- tagged release workflow refuses to publish without signing credentials;
- the full current test and validation matrix remains green.

## Decisions Recorded

- GitHub OAuth App device flow with the registered public client ID and `repo`
  scope.
- Polar Observatory visual direction.
- Native title bar and controls.
- Close exits; tray provides Open and Quit.
- Windows x64, universal macOS, and Linux x86_64 for the first release.
- Unsigned development bundles are allowed; unsigned official releases are not.
- M3 and M4 behavior remains out of M2.
