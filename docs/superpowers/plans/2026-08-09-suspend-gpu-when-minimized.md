# Suspend GPU Rendering While Minimized Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Stop Stellr's canvas animation loop while its native window is minimized, retain five-minute background polling, and resume the preserved map immediately on restore.

**Architecture:** The imperative canvas renderer owns an idempotent `suspend()`/`resume()` state machine and a logical animation clock. The existing native-shell frontend seam translates Tauri minimized state or browser document visibility into a boolean lifecycle callback, while the Svelte wrapper connects that callback to the renderer without remounting it. Rust polling remains unchanged because it already switches an unfocused native window from 30-second to five-minute polling.

**Tech Stack:** TypeScript 6, Svelte 5, Tauri 2.11 window API, Vitest 4 with jsdom, native Windows PowerShell, Vite+, Rust/Cargo workspace validation.

## Global Constraints

- Use only native Windows commands and executables; do not use WSL, Linux shells, or `/mnt/*` paths.
- Work in `D:\tmp\stellr-suspend-gpu-when-minimized` on branch `codex/suspend-gpu-when-minimized`; do not modify `D:\dev\stellr` or its current branch.
- Install frontend dependencies with `vp.exe install --frozen-lockfile` from `web` if `web\node_modules` is absent.
- Write each production change only after its focused test fails for the expected missing behavior.
- A minimized or hidden map schedules zero canvas animation frames; restore schedules exactly one loop.
- Minimize/restore preserves the renderer instance, model, layout, camera, selection, flare, ticker, and animation phase.
- Native polling remains 30 seconds while focused and five minutes while minimized or otherwise unfocused; manual refresh remains immediate.
- Browser `serve` mode retains its existing transport and polling behavior and uses `document.hidden` only to suspend rendering.
- Do not add dependencies, a user-facing power setting, tray-hide behavior, WebSocket suspension, or Rust-to-web lifecycle events.
- Add the release note once under `CHANGELOG.md`'s `Unreleased` section; do not edit shipped release sections.

---

## File Map

- `web/src/lib/starmap/starmap.ts`: owns renderer scheduling, suspension state, and the logical animation/ticker clock.
- `web/src/lib/starmap/render-lifecycle.test.ts`: focused real-renderer tests for frame cancellation, idempotent resume, state preservation, and frozen logical time.
- `web/src/lib/native-shell.ts`: owns the Tauri/browser window-suspension observer beside the existing native-shell adapters.
- `web/src/lib/native-shell.test.ts`: tests native minimized queries, stale async result rejection, fail-open behavior, browser visibility, and cleanup.
- `web/src/lib/StarMap.svelte`: mounts the renderer initially suspended and connects it to the lifecycle observer.
- `web/src/lib/StarMap.test.ts`: proves wrapper wiring and teardown through the real browser visibility seam.
- `CHANGELOG.md`: records the pending power-saving behavior under `Unreleased`.

---

### Task 1: Make the Canvas Renderer Suspendable

**Files:**
- Create: `web/src/lib/starmap/render-lifecycle.test.ts`
- Modify: `web/src/lib/starmap/starmap.ts:250-326`
- Modify: `web/src/lib/starmap/starmap.ts:517-527`
- Modify: `web/src/lib/starmap/starmap.ts:639-649`
- Modify: `web/src/lib/starmap/starmap.ts:753-776`
- Modify: `web/src/lib/starmap/starmap.ts:908-929`

**Interfaces:**
- Consumes: the existing `StarMap.mount(host)`, `setModel(...)`, `restoreCamera(...)`, `camera()`, `positions()`, `ticker()`, and `destroy()` renderer seams.
- Produces: `StarMap.suspend(): void` and `StarMap.resume(): void`; both are idempotent and safe before mount. `resume()` schedules only when a live 2D context exists.

- [ ] **Step 1: Install the locked frontend dependencies if this worktree has none**

Run from `D:\tmp\stellr-suspend-gpu-when-minimized\web`:

```powershell
if (!(Test-Path -LiteralPath node_modules)) { vp.exe install --frozen-lockfile }
```

Expected: dependencies install without changing `package-lock.json`; if already installed, the command makes no change.

- [ ] **Step 2: Write focused renderer lifecycle tests**

Create `src/lib/starmap/render-lifecycle.test.ts` with a real `StarMap`, controlled animation-frame queue, fake `performance.now()`, and a canvas context that accepts the renderer's actual draw calls:

```ts
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import type { Ticket } from './model'
import { StarMap } from './starmap'

const ticket = (status: Ticket['status'] = 'open'): Ticket => ({
  num: 1,
  slug: '1',
  title: 'Power-saving map',
  type: 'task',
  status,
  blockedBy: [],
  parentIssue: null,
  frontier: status === 'open',
})

function drawingContext(paint: () => void): CanvasRenderingContext2D {
  const values: Record<PropertyKey, unknown> = {
    createRadialGradient: () => ({ addColorStop: () => undefined }),
    fillRect: paint,
    measureText: () => ({ width: 40 }),
  }
  return new Proxy(values, {
    get(target, property) {
      if (property in target) return target[property]
      const method = () => undefined
      target[property] = method
      return method
    },
    set(target, property, value) {
      target[property] = value
      return true
    },
  }) as unknown as CanvasRenderingContext2D
}

describe('render lifecycle', () => {
  let nextFrame: number
  let frames: Map<number, FrameRequestCallback>
  let hostWidth: number
  let paint: ReturnType<typeof vi.fn>
  let resize: ResizeObserverCallback

  beforeEach(() => {
    nextFrame = 1
    frames = new Map()
    hostWidth = 1000
    paint = vi.fn()
    vi.useFakeTimers({ toFake: ['performance'] })
    vi.spyOn(HTMLCanvasElement.prototype, 'getContext').mockReturnValue(drawingContext(paint))
    vi.stubGlobal('ResizeObserver', class {
      constructor(callback: ResizeObserverCallback) {
        resize = callback
      }
      observe(): void {}
      disconnect(): void {}
    })
    vi.stubGlobal('requestAnimationFrame', vi.fn((callback: FrameRequestCallback) => {
      const id = nextFrame++
      frames.set(id, callback)
      return id
    }))
    vi.stubGlobal('cancelAnimationFrame', vi.fn((id: number) => {
      frames.delete(id)
    }))
  })

  afterEach(() => {
    vi.useRealTimers()
    vi.unstubAllGlobals()
    vi.restoreAllMocks()
    document.body.innerHTML = ''
  })

  function mounted(): StarMap {
    const host = document.createElement('div')
    Object.defineProperty(host, 'clientWidth', { get: () => hostWidth })
    Object.defineProperty(host, 'clientHeight', { value: 700 })
    document.body.appendChild(host)
    const renderer = new StarMap()
    renderer.mount(host)
    return renderer
  }

  function step(milliseconds = 16): void {
    vi.advanceTimersByTime(milliseconds)
    const entry = frames.entries().next().value as [number, FrameRequestCallback] | undefined
    if (!entry) throw new Error('expected a scheduled animation frame')
    frames.delete(entry[0])
    entry[1](performance.now())
  }

  it('cancels every frame while suspended and resumes exactly one loop', () => {
    const renderer = mounted()
    expect(frames.size).toBe(1)

    renderer.suspend()
    expect(frames.size).toBe(0)
    renderer.suspend()
    expect(frames.size).toBe(0)

    renderer.resume()
    expect(frames.size).toBe(1)
    renderer.resume()
    expect(frames.size).toBe(1)

    step()
    expect(frames.size).toBe(1)
    renderer.destroy()
    expect(frames.size).toBe(0)
  })

  it('can mount suspended and preserves map state until resume', () => {
    const renderer = new StarMap()
    renderer.suspend()
    const host = document.createElement('div')
    Object.defineProperty(host, 'clientWidth', { value: 1000 })
    Object.defineProperty(host, 'clientHeight', { value: 700 })
    document.body.appendChild(host)
    renderer.mount(host)
    renderer.setModel([ticket()])
    renderer.select(1)
    renderer.restoreCamera({ x: 120, y: 180, s: 1.5 })

    expect(frames.size).toBe(0)
    expect(renderer.positions()[1]).toBeDefined()
    expect(renderer.camera()).toEqual({ x: 120, y: 180, s: 1.5 })

    renderer.resume()
    expect(frames.size).toBe(1)
    expect(renderer.positions()[1]).toBeDefined()
    expect(renderer.camera()).toEqual({ x: 120, y: 180, s: 1.5 })
  })

  it('does not repaint from resize notifications while suspended', () => {
    const renderer = mounted()
    renderer.suspend()
    hostWidth = 800

    resize([], {} as ResizeObserver)

    expect(paint).not.toHaveBeenCalled()
    renderer.resume()
    step()
    expect(paint).toHaveBeenCalled()
  })

  it('freezes animation and ticker time for the full suspension', () => {
    const renderer = mounted()
    renderer.setModel([ticket()])
    step()
    renderer.suspend()
    renderer.setModel([ticket('claimed')])
    const ticker = renderer.ticker()

    vi.advanceTimersByTime(10 * 60 * 1000)
    expect(renderer.ticker()).toBe(ticker)

    renderer.resume()
    step()
    expect(renderer.ticker()).toBe(ticker)
  })
})
```

- [ ] **Step 3: Run the focused test and verify RED**

Run from `web`:

```powershell
vp.exe test run src/lib/starmap/render-lifecycle.test.ts
```

Expected: FAIL because `StarMap` has no `suspend()` or `resume()` methods. Do not implement until the failure is confirmed to be this missing behavior.

- [ ] **Step 4: Implement the minimal renderer state machine**

In `starmap.ts`, add the suspension flag beside `#raf`, prevent a suspended mount from scheduling, expose the two lifecycle methods, use logical time for the ticker, and schedule a successor only after an active frame completes:

```ts
#clock = 0
#last = 0
#raf = 0
#suspended = false
```

```ts
if (this.#ctx && !this.#suspended) {
  this.#last = now()
  this.#raf = requestAnimationFrame(this.#render)
}
```

```ts
suspend(): void {
  if (this.#suspended) return
  this.#suspended = true
  if (this.#raf) cancelAnimationFrame(this.#raf)
  this.#raf = 0
}

resume(): void {
  if (!this.#suspended) return
  this.#suspended = false
  this.#last = now()
  if (this.#ctx && !this.#raf) this.#raf = requestAnimationFrame(this.#render)
}
```

Change ticker timestamps from wall time to the renderer's logical clock:

```ts
#tick(msg: string): void {
  this.#tickerText = msg
  this.#tickerAt = this.#clock
}

#tickerAlpha(): number {
  if (!this.#tickerText) return 0
  const age = this.#clock - this.#tickerAt
  if (age < TICKER_HOLD) return 1
  return clamp(1 - (age - TICKER_HOLD) / TICKER_FADE, 0, 1)
}
```

Guard the out-of-band resize repaint while retaining its measurement and camera
updates for the first restored frame:

```ts
if (!this.#suspended) this.#draw()
```

Place that condition where `#onResize()` currently calls `this.#draw()`.

Replace `#render` with this ordering so a callback racing with cancellation cannot restart the loop:

```ts
#render = (): void => {
  this.#raf = 0
  if (this.#suspended || !this.#ctx) return
  const t = now()
  let dt = t - this.#last
  if (dt < 0 || dt > 0.1) dt = 0.016
  this.#last = t
  this.#clock += dt

  for (const n of this.#nodes) {
    const ph = n.num * 1.7
    n._x = n.x + Math.sin(this.#clock * 0.7 + ph) * 2.4
    n._y = n.y + Math.cos(this.#clock * 0.55 + ph) * 2.4
    if (n.flare > 0) n.flare = Math.max(0, n.flare - dt / 1.1)
  }
  this.#easeCamera(dt)
  this.#draw()
  if (!this.#suspended && this.#ctx) this.#raf = requestAnimationFrame(this.#render)
}
```

Keep `destroy()`'s existing frame cancellation and context cleanup unchanged.

- [ ] **Step 5: Run the focused test and existing renderer tests to verify GREEN**

```powershell
vp.exe test run src/lib/starmap/render-lifecycle.test.ts src/lib/starmap/starmap.test.ts src/lib/starmap/edge-visual.test.ts
```

Expected: all focused and existing renderer tests PASS with no warnings.

- [ ] **Step 6: Commit the renderer lifecycle**

```powershell
git add web/src/lib/starmap/starmap.ts web/src/lib/starmap/render-lifecycle.test.ts
git commit -m "feat(web): suspend minimized map rendering"
```

---

### Task 2: Translate Native and Browser Visibility Into Suspension State

**Files:**
- Create: `web/src/lib/native-shell.test.ts`
- Modify: `web/src/lib/native-shell.ts:1-34`

**Interfaces:**
- Consumes: `isTauri()` from `@tauri-apps/api/core`; `getCurrentWindow().isMinimized()`, `onResized(...)`, and `onFocusChanged(...)` from `@tauri-apps/api/window`; `document.hidden` and `visibilitychange` in browser mode.
- Produces: `observeWindowSuspension(notify: (suspended: boolean) => void): Promise<UnlistenFn>`. It emits only changed state, rejects stale native queries, fails open with `false`, and always resolves to cleanup.

- [ ] **Step 1: Write the lifecycle observer tests**

Create `web/src/lib/native-shell.test.ts`:

```ts
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'

const native = vi.hoisted(() => ({
  active: false,
  minimized: vi.fn<() => Promise<boolean>>(),
  onResized: vi.fn(),
  onFocusChanged: vi.fn(),
  resize: undefined as (() => void) | undefined,
  focus: undefined as (() => void) | undefined,
  offResize: vi.fn(),
  offFocus: vi.fn(),
}))

vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(),
  isTauri: () => native.active,
}))

vi.mock('@tauri-apps/api/window', () => ({
  getCurrentWindow: () => ({
    isMinimized: native.minimized,
    onResized: native.onResized,
    onFocusChanged: native.onFocusChanged,
  }),
}))

import { observeWindowSuspension } from './native-shell'

beforeEach(() => {
  native.active = false
  native.resize = undefined
  native.focus = undefined
  native.minimized.mockReset()
  native.onResized.mockReset().mockImplementation(async (handler: () => void) => {
    native.resize = handler
    return native.offResize
  })
  native.onFocusChanged.mockReset().mockImplementation(async (handler: () => void) => {
    native.focus = handler
    return native.offFocus
  })
  native.offResize.mockReset()
  native.offFocus.mockReset()
})

afterEach(() => {
  vi.restoreAllMocks()
})

describe('window rendering suspension', () => {
  it('observes browser document visibility and cleans up', async () => {
    let hidden = false
    vi.spyOn(document, 'hidden', 'get').mockImplementation(() => hidden)
    const states: boolean[] = []
    const stop = await observeWindowSuspension((state) => states.push(state))

    hidden = true
    document.dispatchEvent(new Event('visibilitychange'))
    document.dispatchEvent(new Event('visibilitychange'))
    expect(states).toEqual([false, true])

    stop()
    hidden = false
    document.dispatchEvent(new Event('visibilitychange'))
    expect(states).toEqual([false, true])
  })

  it('queries native minimized state on startup, resize, and focus changes', async () => {
    native.active = true
    native.minimized.mockResolvedValueOnce(true).mockResolvedValueOnce(false).mockResolvedValueOnce(true)
    const states: boolean[] = []
    const stop = await observeWindowSuspension((state) => states.push(state))
    expect(states).toEqual([true])

    native.resize?.()
    await vi.waitFor(() => expect(states).toEqual([true, false]))
    native.focus?.()
    await vi.waitFor(() => expect(states).toEqual([true, false, true]))

    stop()
    expect(native.offResize).toHaveBeenCalledOnce()
    expect(native.offFocus).toHaveBeenCalledOnce()
  })

  it('discards an older native query that resolves after a newer query', async () => {
    native.active = true
    native.minimized.mockResolvedValueOnce(false)
    const states: boolean[] = []
    await observeWindowSuspension((state) => states.push(state))

    let resolveOld!: (value: boolean) => void
    let resolveNew!: (value: boolean) => void
    native.minimized
      .mockReturnValueOnce(new Promise((resolve) => { resolveOld = resolve }))
      .mockReturnValueOnce(new Promise((resolve) => { resolveNew = resolve }))
    native.resize?.()
    native.focus?.()
    resolveNew(false)
    await Promise.resolve()
    resolveOld(true)
    await Promise.resolve()

    expect(states).toEqual([false])
  })

  it('fails open when native minimized state cannot be queried', async () => {
    native.active = true
    native.minimized.mockRejectedValue(new Error('window state unavailable'))
    const states: boolean[] = []

    await observeWindowSuspension((state) => states.push(state))

    expect(states).toEqual([false])
  })

  it('cleans up and fails open when native listener setup fails', async () => {
    native.active = true
    native.onFocusChanged.mockRejectedValueOnce(new Error('focus listener unavailable'))
    const states: boolean[] = []

    const stop = await observeWindowSuspension((state) => states.push(state))

    expect(states).toEqual([false])
    expect(native.offResize).toHaveBeenCalledOnce()
    expect(() => stop()).not.toThrow()
  })
})
```

- [ ] **Step 2: Run the observer test and verify RED**

Run from `web`:

```powershell
vp.exe test run src/lib/native-shell.test.ts
```

Expected: FAIL because `observeWindowSuspension` is not exported.

- [ ] **Step 3: Implement the native/browser observer**

Add imports and the observer to `native-shell.ts` without changing the existing theme, directory, or external-link functions:

```ts
import type { UnlistenFn } from '@tauri-apps/api/event'
import { getCurrentWindow } from '@tauri-apps/api/window'

export async function observeWindowSuspension(
  notify: (suspended: boolean) => void,
): Promise<UnlistenFn> {
  if (!isTauri()) {
    let lastState: boolean | undefined
    const publish = () => {
      if (lastState === document.hidden) return
      lastState = document.hidden
      notify(lastState)
    }
    document.addEventListener('visibilitychange', publish)
    publish()
    return () => document.removeEventListener('visibilitychange', publish)
  }

  const window = getCurrentWindow()
  const unlisteners: UnlistenFn[] = []
  let disposed = false
  let revision = 0
  let lastState: boolean | undefined
  const publish = (state: boolean) => {
    if (lastState === state) return
    lastState = state
    notify(state)
  }
  const refresh = async () => {
    const request = ++revision
    let minimized = false
    try {
      minimized = await window.isMinimized()
    } catch {
      minimized = false
    }
    if (!disposed && request === revision) publish(minimized)
  }

  try {
    unlisteners.push(await window.onResized(() => { void refresh() }))
    unlisteners.push(await window.onFocusChanged(() => { void refresh() }))
    await refresh()
  } catch {
    disposed = true
    revision++
    for (const unlisten of unlisteners) unlisten()
    publish(false)
    return () => undefined
  }

  return () => {
    disposed = true
    revision++
    for (const unlisten of unlisteners) unlisten()
  }
}
```

- [ ] **Step 4: Run native-shell and related frontend tests to verify GREEN**

```powershell
vp.exe test run src/lib/native-shell.test.ts src/lib/native-route.test.ts src/lib/AppearanceMenu.test.ts
```

Expected: all tests PASS, including cleanup, stale-result, and fail-open cases.

- [ ] **Step 5: Commit the lifecycle observer**

```powershell
git add web/src/lib/native-shell.ts web/src/lib/native-shell.test.ts
git commit -m "feat(web): observe minimized window state"
```

---

### Task 3: Wire Suspension Into the Svelte Wrapper and Complete Validation

**Files:**
- Modify: `web/src/lib/StarMap.test.ts:1-184`
- Modify: `web/src/lib/StarMap.svelte:1-57`
- Modify: `CHANGELOG.md:3-6`

**Interfaces:**
- Consumes: `observeWindowSuspension(...)`, `StarMap.suspend()`, `StarMap.resume()`, and `StarMap.destroy()` from Tasks 1 and 2.
- Produces: a wrapper that never starts a native render loop before initial minimized state is known, applies every lifecycle notification to the same renderer instance, and unregisters both observer and renderer resources on unmount.

- [ ] **Step 1: Write the failing wrapper lifecycle test**

Add this asynchronous test inside `describe('StarMap wrapper', ...)` in `StarMap.test.ts`:

```ts
it('suspends rendering with document visibility and removes the observer on unmount', async () => {
  let hidden = false
  vi.spyOn(document, 'hidden', 'get').mockImplementation(() => hidden)
  const suspend = vi.spyOn(Renderer.prototype, 'suspend')
  const resume = vi.spyOn(Renderer.prototype, 'resume')
  const destroy = vi.spyOn(Renderer.prototype, 'destroy')
  const target = document.createElement('div')
  document.body.appendChild(target)
  const component = mount(StarMap, { target, props: { space: space(42) } })
  flushSync()
  await Promise.resolve()

  expect(suspend).toHaveBeenCalledOnce()
  expect(resume).toHaveBeenCalledOnce()

  hidden = true
  document.dispatchEvent(new Event('visibilitychange'))
  expect(suspend).toHaveBeenCalledTimes(2)

  hidden = false
  document.dispatchEvent(new Event('visibilitychange'))
  expect(resume).toHaveBeenCalledTimes(2)

  await unmount(component)
  hidden = true
  document.dispatchEvent(new Event('visibilitychange'))
  expect(suspend).toHaveBeenCalledTimes(2)
  expect(destroy).toHaveBeenCalledOnce()
})
```

Do not add this component to the shared `mounted` array because the test unmounts it explicitly before verifying listener cleanup.

- [ ] **Step 2: Run the wrapper test and verify RED**

```powershell
vp.exe test run src/lib/StarMap.test.ts -t "suspends rendering with document visibility"
```

Expected: FAIL because the wrapper does not call `suspend()` or `resume()` and has no visibility observer.

- [ ] **Step 3: Connect the wrapper to the lifecycle seam**

Import `observeWindowSuspension` and replace the current `onMount` body in `StarMap.svelte` with lifecycle-safe setup and cleanup:

```ts
import { observeWindowSuspension } from './native-shell'
```

```ts
onMount(() => {
  let disposed = false
  let stopObserving: (() => void) | undefined
  const activeRenderer = new Renderer()
  renderer = activeRenderer
  activeRenderer.suspend()
  const background = getComputedStyle(host).getPropertyValue('--map-background').trim()
  activeRenderer.setBackground(background)
  activeRenderer.mount(host)
  activeRenderer.onSelect((issueNumber) => {
    if (issueNumber !== null && issueNumber !== selectedIssue) select?.(issueNumber)
  })

  void observeWindowSuspension((suspended) => {
    if (disposed) return
    if (suspended) activeRenderer.suspend()
    else activeRenderer.resume()
  }).then((unlisten) => {
    if (disposed) unlisten()
    else stopObserving = unlisten
  })

  return () => {
    disposed = true
    stopObserving?.()
    activeRenderer.destroy()
    if (renderer === activeRenderer) renderer = undefined
  }
})
```

- [ ] **Step 4: Add the append-only release note**

Add this newest bullet directly below `## Unreleased` in `CHANGELOG.md`:

```markdown
- Suspended star-map GPU rendering while the app is minimized or its browser
  document is hidden, while retaining five-minute native background polling.
```

- [ ] **Step 5: Run the wrapper test and complete frontend verification**

Run from `web`:

```powershell
vp.exe test run src/lib/StarMap.test.ts -t "suspends rendering with document visibility"
vp.exe test run
vp.exe run check
vp.exe build
```

Expected: the focused test passes; the full frontend suite passes; Svelte check reports zero errors and warnings; the production build succeeds.

- [ ] **Step 6: Complete native Windows Rust verification**

Run from the worktree root after `web\dist` exists:

```powershell
cargo.exe fmt --all -- --check
cargo.exe clippy --workspace --all-targets --locked -- -D warnings
cargo.exe test --workspace --locked -- --test-threads=1
```

Expected: formatting passes, Clippy emits no warnings, and every locked workspace test passes serially. The existing focus-aware polling tests remain green, proving the 30-second/five-minute policy was not changed.

- [ ] **Step 7: Inspect the final diff and commit the wrapper and release note**

```powershell
git diff --check
git status --short
git diff -- web/src/lib/StarMap.svelte web/src/lib/StarMap.test.ts CHANGELOG.md
git add web/src/lib/StarMap.svelte web/src/lib/StarMap.test.ts CHANGELOG.md
git commit -m "feat(web): pause rendering while minimized"
```

Expected: only the scoped feature files and already committed plan/spec files differ from `main`; the worktree is clean after the commit.

---

## Completion Evidence

Before claiming completion, record:

- the RED failure for each of the three focused test files before production changes;
- the focused GREEN commands after each task;
- the final frontend test count and Svelte check/build results;
- native `cargo fmt`, warnings-denied Clippy, and serialized locked workspace-test results;
- `git status --short --branch` and the final commit list on `codex/suspend-gpu-when-minimized`.

Do not claim measured zero GPU utilization without a real packaged Windows-process measurement. The automated acceptance claim is narrower and exact: no canvas animation frame remains scheduled while minimized/hidden, and one loop resumes afterward.
