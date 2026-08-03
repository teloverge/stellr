# stellr

**A star-map of your GitHub issues, and an agent multiplexer to work them.**

Stellr charts repository issues as an interactive star-map: blocking
relationships become edges, milestones become clusters, and completed work
becomes resolved stars. The native shell and the browser-hosted serve mode use
the same Rust application runtime.

**Status: M2 native shell available.** Windows 11, macOS, and Linux desktop
packages are built on native CI runners. The approved product design remains in
[`docs/specs/2026-07-29-stellr-port-design.md`](docs/specs/2026-07-29-stellr-port-design.md).

## Build from source

Install stable Rust, Node.js 24 with npm, and GitHub CLI. From the repository
root in PowerShell:

```powershell
npm.cmd --prefix web ci
npm.cmd --prefix web run build
cargo.exe build -p stellr-app
```

## Desktop mode

A bare launch opens the native desktop shell and restores the last valid route:

```powershell
cargo.exe run -p stellr-app
```

Open a repository, issue URL, local checkout, or Stellr protocol target
explicitly:

```powershell
cargo.exe run -p stellr-app -- open teloverge/stellr
cargo.exe run -p stellr-app -- open https://github.com/teloverge/stellr/issues/70
cargo.exe run -p stellr-app -- open D:\dev\stellr
```

Packaged installations use the same commands through the `stellr` executable.
Only one desktop process runs at a time; later invocations forward their target
to the existing window, restore it if minimized, focus it, and exit.

## Credential precedence

GitHub provider credentials resolve in this order:

1. a nonblank `GITHUB_TOKEN` environment variable;
2. the token returned by `gh auth token`;
3. the operating-system credential store entry for service `stellr.github` and
   account `default`;
4. an unauthenticated desktop state that offers device authorization.

The provider credential is separate from the random per-run browser session
token printed by serve mode. Neither token is exposed to the webview URL.

## Device authorization

When desktop mode cannot resolve a credential, it still opens the native shell
and presents **Connect GitHub**. Start authorization, open the supplied GitHub
verification URL in the system browser, and enter the one-time code. Stellr
requests the approved `repo` scope, activates synchronization immediately, and
stores the resulting credential in the operating-system credential store. If
storage fails, the current run remains connected and the shell reports that the
next launch will require sign-in again.

Serve mode does not start device authorization because it has no trusted native
interaction surface; configure one of the first three credential sources before
starting it.

## Serve mode

Serve the same embedded application over loopback for a browser or IDE pane:

```powershell
cargo.exe run -p stellr-app -- serve
cargo.exe run -p stellr-app -- serve --addr 127.0.0.1:0 --issue 70
```

Open the printed `stellr cockpit` URL. Protected API and control-WebSocket
routes require the generated session token by default. `--no-token` is an
explicit local-development option; do not expose that listener to another host.

## Deep links and single-instance routing

Desktop targets normalize through one route model. Supported forms include:

- local repository paths;
- `owner/repo` GitHub slugs;
- canonical GitHub repository and issue URLs;
- `stellr://space?repo=owner%2Frepo&issue=70` links registered by packages.

Explicit targets override restored state. A bare launch restores the last valid
space and issue while rejecting corrupt or off-origin route data.

## Supported packages

| Platform | Supported package |
| --- | --- |
| Windows 11 x64 | NSIS installer with WebView2 bootstrap support |
| macOS | Universal DMG containing both Apple Silicon and Intel slices |
| Linux x86_64 | AppImage and Debian package |

Pull-request and manual packaging workflows label their outputs
`UNSIGNED-NOT-FOR-RELEASE`. Tagged release candidates fail before publication
unless Windows and macOS signing credentials are configured, pass each native
install/inspection/launch gate, and complete the full repository validation
suite.

## Lineage and acknowledgements

Stellr is a Rust/Tauri port and generalization of
[chartr](https://github.com/rengwu/chartr) (Go, MIT, copyright 2026 John Goh).
Ported code retains the chartr notice in [`LICENSE-chartr`](LICENSE-chartr).

Further upstream:

- [wayfinder-maps](https://github.com/rengwu/wayfinder-maps), where the star-map
  started;
- [herdr](https://github.com/ogulcancelik/herdr), the terminal agent
  multiplexer that inspired chartr;
- [mattpocock/skills](https://github.com/mattpocock/skills), the original
  `/wayfinder` planning workflow.

## License

MIT; see [`LICENSE`](LICENSE) and the retained [`LICENSE-chartr`](LICENSE-chartr).
