# GNOME Support Implementation Plan

> **Superseded in places during implementation and review.** This plan is a
> historical execution artifact; the design spec
> (`docs/superpowers/specs/2026-07-20-gnome-support-design.md`) is the
> authoritative description of shipped behavior. Notable divergences:
> `backend_for_desktop` is now infallible (`-> Backend`, case-insensitive,
> falling back to KWin on unknown/unset desktops instead of erroring); the
> per-backend `call_with_timeout` blocks were consolidated into a shared
> `timeout::call_dbus_with_timeout`; query matching is token-based rather
> than whole-substring; the workspace subtext is 1-based; and `List` records
> are parsed per-window so one malformed entry doesn't abort the list.

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add `GnomeBackend`, a second `WindowBackend` implementation talking to the "Window Calls" GNOME Shell extension over D-Bus, and make `detect_backend()` pick between it and `KWinBackend` by `XDG_CURRENT_DESKTOP`.

**Architecture:** `GnomeBackend` mirrors `KWinBackend`'s shape — one D-Bus interface (`org.gnome.Shell.Extensions.Windows`), `list_windows`/`activate` wrapped in the same timeout pattern (`crate::timeout::run_with_timeout`). Since `List()` returns a JSON string rather than a native D-Bus array, `list_windows` deserializes it with `serde_json` into a `Vec<GnomeWindow>` and filters client-side (case-insensitive substring match on title/wm_class) — KWin does relevance matching server-side via `Match()`, GNOME has no equivalent so `was` does it itself. `detect_backend()`'s KDE-vs-GNOME choice is extracted into a pure `backend_for_desktop(&str) -> Result<Backend, String>` function so the dispatch logic is unit-testable without any D-Bus connection.

**Tech Stack:** Same as the rest of the project — Rust 2024, `zbus` (blocking, `async-io`), `anyhow`. Adds `serde`/`serde_json` for parsing `List()`'s JSON payload.

## Global Constraints

- `GnomeBackend`'s real D-Bus calls are NOT exercised in this environment — this dev machine is KDE Plasma, with no GNOME/Mutter session available. Only the pure logic (filtering, mapping, dispatch) gets automated tests; the real D-Bus integration is manually-unverified until run on an actual GNOME session (spec: Testing).
- Empty query means "all windows", matching the existing `WindowBackend` contract and `KWinBackend`'s behavior (spec: Matching semantics).
- Missing/disabled "Window Calls" extension must produce an error naming the extension and pointing at how to install it, not a generic/confusing D-Bus error (spec: Error handling).
- Unsupported desktop (`XDG_CURRENT_DESKTOP` matches neither KDE nor GNOME): clear error naming the detected value, exit 1, no panic (spec: Error handling).

---

## File Structure

- `Cargo.toml` — add `serde` (`derive` feature) and `serde_json`.
- `src/backend/gnome.rs` — new. `GnomeWindow` (private JSON-shape struct), `window_info_from_gnome_window`, `matches_query`, `GnomeBackend` (real D-Bus `WindowBackend` impl).
- `src/backend/mod.rs` — modify: add `pub mod gnome;`, `pub enum Backend`, `pub fn backend_for_desktop`, rewrite `detect_backend()` to dispatch through it.

---

## Task 1: `GnomeWindow` parsing, filtering, and `WindowInfo` mapping (TDD core)

**Files:**
- Modify: `Cargo.toml`
- Create: `src/backend/gnome.rs` (only the pure parts in this task — no real D-Bus yet)
- Modify: `src/backend/mod.rs` (add `pub mod gnome;`)

**Interfaces:**
- Consumes: `WindowInfo` from `src/backend/mod.rs` (already exists: `{ id: String, title: String, icon: String, subtext: String }`).
- Produces:
  ```rust
  // src/backend/gnome.rs
  #[derive(serde::Deserialize)]
  struct GnomeWindow {
      id: u32,
      title: String,
      wm_class: String,
      workspace: i32,
  }

  fn matches_query(w: &GnomeWindow, query: &str) -> bool;
  fn window_info_from_gnome_window(w: GnomeWindow) -> WindowInfo;
  ```

- [ ] **Step 1: Add `serde`/`serde_json` to `Cargo.toml`**

Add to the `[dependencies]` section:

```toml
serde = { version = "1", features = ["derive"] }
serde_json = "1"
```

- [ ] **Step 2: Write the failing tests in `src/backend/gnome.rs`**

```rust
use crate::backend::WindowInfo;

#[derive(serde::Deserialize)]
struct GnomeWindow {
    id: u32,
    title: String,
    wm_class: String,
    workspace: i32,
}

fn matches_query(w: &GnomeWindow, query: &str) -> bool {
    unimplemented!()
}

fn window_info_from_gnome_window(w: GnomeWindow) -> WindowInfo {
    unimplemented!()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn window(id: u32, title: &str, wm_class: &str, workspace: i32) -> GnomeWindow {
        GnomeWindow { id, title: title.to_string(), wm_class: wm_class.to_string(), workspace }
    }

    #[test]
    fn empty_query_matches_everything() {
        let w = window(1, "Firefox", "firefox", 0);
        assert!(matches_query(&w, ""));
    }

    #[test]
    fn query_matches_title_case_insensitively() {
        let w = window(1, "Mozilla Firefox", "firefox", 0);
        assert!(matches_query(&w, "FIREFOX"));
    }

    #[test]
    fn query_matches_wm_class_case_insensitively() {
        let w = window(1, "some window", "Konsole", 0);
        assert!(matches_query(&w, "konsole"));
    }

    #[test]
    fn query_matching_neither_field_is_rejected() {
        let w = window(1, "Firefox", "firefox", 0);
        assert!(!matches_query(&w, "konsole"));
    }

    #[test]
    fn maps_fields_and_synthesizes_subtext_from_workspace() {
        let w = window(42, "My Window", "myapp", 3);
        let info = window_info_from_gnome_window(w);
        assert_eq!(info.id, "42");
        assert_eq!(info.title, "My Window");
        assert_eq!(info.icon, "");
        assert_eq!(info.subtext, "Activate running window on workspace 3");
    }

    #[test]
    fn deserializes_list_json_ignoring_unknown_fields() {
        let json = r#"[
            {"id": 7, "title": "Term", "wm_class": "Konsole", "workspace": 1,
             "in_current_workspace": true, "pid": 123, "wm_class_instance": "konsole",
             "frame_type": 0, "window_type": 0, "width": 800, "height": 600,
             "x": 0, "y": 0, "focus": true}
        ]"#;
        let windows: Vec<GnomeWindow> = serde_json::from_str(json).unwrap();
        assert_eq!(windows.len(), 1);
        assert_eq!(windows[0].id, 7);
        assert_eq!(windows[0].title, "Term");
    }
}
```

- [ ] **Step 3: Add `pub mod gnome;` to `src/backend/mod.rs`**

Add near the existing `pub mod kwin;` line:

```rust
pub mod gnome;
pub mod kwin;
```

- [ ] **Step 4: Run tests to verify the two `unimplemented!()` functions fail**

Run: `cargo test gnome::`
Expected: `query_matches_title_case_insensitively`, `query_matches_wm_class_case_insensitively`, `query_matching_neither_field_is_rejected`, `empty_query_matches_everything`, and `maps_fields_and_synthesizes_subtext_from_workspace` panic on `unimplemented!()`. `deserializes_list_json_ignoring_unknown_fields` already passes (it doesn't call either unimplemented function).

- [ ] **Step 5: Implement `matches_query` and `window_info_from_gnome_window`**

```rust
fn matches_query(w: &GnomeWindow, query: &str) -> bool {
    if query.is_empty() {
        return true;
    }
    let query = query.to_lowercase();
    w.title.to_lowercase().contains(&query) || w.wm_class.to_lowercase().contains(&query)
}

fn window_info_from_gnome_window(w: GnomeWindow) -> WindowInfo {
    WindowInfo {
        id: w.id.to_string(),
        title: w.title,
        icon: String::new(),
        subtext: format!("Activate running window on workspace {}", w.workspace),
    }
}
```

- [ ] **Step 6: Run tests to verify they pass**

Run: `cargo test gnome::`
Expected: all 6 tests in `backend::gnome::tests` PASS.

- [ ] **Step 7: Commit**

```bash
git add Cargo.toml Cargo.lock src/backend/gnome.rs src/backend/mod.rs
git commit -m "Add GnomeWindow parsing, filtering, and WindowInfo mapping"
```

---

## Task 2: `GnomeBackend` — real D-Bus `WindowBackend` implementation

**Files:**
- Modify: `src/backend/gnome.rs` (add `GnomeBackend`, imports, D-Bus call wiring)

**Interfaces:**
- Consumes: `WindowBackend` trait, `WindowInfo` (`src/backend/mod.rs`); `run_with_timeout`, `TimeoutError` (`src/timeout.rs`); `matches_query`, `window_info_from_gnome_window`, `GnomeWindow` (Task 1, same file).
- Produces: `pub struct GnomeBackend { .. }` with `pub fn connect() -> anyhow::Result<Self>` and `impl WindowBackend for GnomeBackend`.

No automated tests for the real D-Bus calls (no GNOME session available in
this environment); verified only by `cargo build` succeeding. Manual
verification must happen later on a real GNOME/Mutter session with the
"Window Calls" extension installed.

- [ ] **Step 1: Add imports and `GnomeBackend` to the top of `src/backend/gnome.rs`**

Insert above the existing `use crate::backend::WindowInfo;` line, replacing it:

```rust
use crate::backend::{WindowBackend, WindowInfo};
use crate::timeout::{run_with_timeout, TimeoutError};
use std::time::Duration;
use zbus::blocking::Connection;

const DEST: &str = "org.gnome.Shell";
const PATH: &str = "/org/gnome/Shell/Extensions/Windows";
const IFACE: &str = "org.gnome.Shell.Extensions.Windows";
const CALL_TIMEOUT: Duration = Duration::from_secs(3);

pub struct GnomeBackend {
    conn: Connection,
}

impl GnomeBackend {
    pub fn connect() -> anyhow::Result<Self> {
        Ok(Self { conn: Connection::session()? })
    }
}

fn call_with_timeout<T: Send + 'static>(
    context: &'static str,
    f: impl FnOnce() -> zbus::Result<T> + Send + 'static,
) -> anyhow::Result<T> {
    match run_with_timeout(CALL_TIMEOUT, f) {
        Ok(Ok(value)) => Ok(value),
        Ok(Err(e)) => anyhow::bail!(
            "{context}: {e} (is the \"Window Calls\" GNOME Shell extension installed \
             and enabled? https://extensions.gnome.org/extension/4724/window-calls/)"
        ),
        Err(TimeoutError::Elapsed) => anyhow::bail!("GNOME Shell is not responding"),
        Err(TimeoutError::WorkerPanicked) => {
            anyhow::bail!("GNOME worker thread panicked or exited unexpectedly")
        }
    }
}

impl WindowBackend for GnomeBackend {
    fn list_windows(&self, query: &str) -> anyhow::Result<Vec<WindowInfo>> {
        let conn = self.conn.clone();
        let reply = call_with_timeout("GNOME List call failed", move || {
            conn.call_method(Some(DEST), PATH, Some(IFACE), "List", &())
        })?;
        let json: String = reply.body().deserialize()?;
        let windows: Vec<GnomeWindow> = serde_json::from_str(&json)?;
        Ok(windows
            .into_iter()
            .filter(|w| matches_query(w, query))
            .map(window_info_from_gnome_window)
            .collect())
    }

    fn activate(&self, id: &str) -> anyhow::Result<()> {
        let winid: u32 = id
            .parse()
            .map_err(|_| anyhow::anyhow!("invalid GNOME window id: {id:?}"))?;
        let conn = self.conn.clone();
        call_with_timeout("GNOME Activate call failed", move || {
            conn.call_method(Some(DEST), PATH, Some(IFACE), "Activate", &(winid,))
        })?;
        Ok(())
    }
}
```

- [ ] **Step 2: Build**

Run: `cargo build`
Expected: compiles. If `zbus::blocking::Connection::call_method`'s generic bounds reject the `&()` (no-argument) call in `list_windows`, check how `KWinBackend`'s existing calls pass arguments (`src/backend/kwin.rs`) for the exact tuple-vs-unit convention zbus 4.x expects here, and adjust to match (e.g. it may need `&()` explicitly typed, which is what's shown above).

- [ ] **Step 3: Run the full test suite to confirm Task 1's tests are untouched**

Run: `cargo test`
Expected: all existing tests plus the 6 `backend::gnome::tests` from Task 1 still pass (this task adds no new automated tests, per the Global Constraints note on GNOME being unverifiable here).

- [ ] **Step 4: Commit**

```bash
git add src/backend/gnome.rs
git commit -m "Add GnomeBackend over org.gnome.Shell.Extensions.Windows (Window Calls)"
```

---

## Task 3: Backend selection by `XDG_CURRENT_DESKTOP`

**Files:**
- Modify: `src/backend/mod.rs`

**Interfaces:**
- Consumes: `KWinBackend::connect()` (`src/backend/kwin.rs`), `GnomeBackend::connect()` (Task 2).
- Produces:
  ```rust
  pub enum Backend { Kde, Gnome }
  pub fn backend_for_desktop(desktop: &str) -> Result<Backend, String>;
  ```
  `detect_backend()`'s signature (`pub fn detect_backend() -> anyhow::Result<Box<dyn WindowBackend>>`) is unchanged — only its body changes.

- [ ] **Step 1: Write the failing tests**

Read the current `src/backend/mod.rs` first — it currently ends with:

```rust
pub fn detect_backend() -> anyhow::Result<Box<dyn WindowBackend>> {
    Ok(Box::new(kwin::KWinBackend::connect()?))
}
```

Replace that function and add the new dispatch logic plus tests, appending after the existing `MatchResolution`/`classify_matches` code (keep everything already in the file — `WindowInfo`, `WindowBackend`, `pub mod kwin;`, `pub mod gnome;`, `MatchResolution`, `classify_matches`, and the existing `#[cfg(test)] mod tests` for those untouched):

```rust
pub enum Backend {
    Kde,
    Gnome,
}

pub fn backend_for_desktop(desktop: &str) -> Result<Backend, String> {
    if desktop.contains("GNOME") {
        Ok(Backend::Gnome)
    } else if desktop.contains("KDE") {
        Ok(Backend::Kde)
    } else {
        Err(format!(
            "unsupported desktop environment (XDG_CURRENT_DESKTOP={desktop:?}); \
             only KDE Plasma and GNOME are supported"
        ))
    }
}

pub fn detect_backend() -> anyhow::Result<Box<dyn WindowBackend>> {
    let desktop = std::env::var("XDG_CURRENT_DESKTOP").unwrap_or_default();
    match backend_for_desktop(&desktop) {
        Ok(Backend::Kde) => Ok(Box::new(kwin::KWinBackend::connect()?)),
        Ok(Backend::Gnome) => Ok(Box::new(gnome::GnomeBackend::connect()?)),
        Err(msg) => anyhow::bail!(msg),
    }
}
```

Add these test cases to the file's existing `#[cfg(test)] mod tests` block (append inside it, alongside `zero_matches_classifies_as_none` etc. — do not create a second `mod tests`):

```rust
#[test]
fn plain_kde_selects_kde_backend() {
    assert!(matches!(backend_for_desktop("KDE"), Ok(Backend::Kde)));
}

#[test]
fn plain_gnome_selects_gnome_backend() {
    assert!(matches!(backend_for_desktop("GNOME"), Ok(Backend::Gnome)));
}

#[test]
fn colon_separated_desktop_list_containing_gnome_selects_gnome() {
    // XDG_CURRENT_DESKTOP can be a colon-separated list, e.g. Ubuntu sets
    // "ubuntu:GNOME" — a substring check must still catch it.
    assert!(matches!(backend_for_desktop("ubuntu:GNOME"), Ok(Backend::Gnome)));
}

#[test]
fn unknown_desktop_is_rejected_with_a_clear_message() {
    let err = backend_for_desktop("XFCE").unwrap_err();
    assert!(err.contains("XFCE"));
}

#[test]
fn empty_desktop_is_rejected() {
    assert!(backend_for_desktop("").is_err());
}
```

- [ ] **Step 2: Run tests to verify the new ones pass immediately (this step is pure refactor + additive logic, not TDD-red-first, since `backend_for_desktop` has no prior implementation to regress)**

Run: `cargo test backend::tests`
Expected: all tests in `backend::tests` pass, including the 5 new ones and the pre-existing `MatchResolution` tests from earlier work.

- [ ] **Step 3: Build and run the full test suite**

Run: `cargo build && cargo test`
Expected: builds; all tests pass (Task 1's 6 gnome tests + Task 3's 5 backend tests + everything from before this feature).

- [ ] **Step 4: Commit**

```bash
git add src/backend/mod.rs
git commit -m "Dispatch detect_backend() by XDG_CURRENT_DESKTOP (KDE vs GNOME)"
```

---

## Task 4: Docs and final verification

**Files:**
- Modify: `README.md`
- Modify: `docs/superpowers/specs/2026-07-20-gnome-support-design.md` (mark the JSON-schema assumption as verified)

- [ ] **Step 1: Update `README.md`'s Requirements and How it works sections**

In the `## Requirements` section, replace:

```markdown
- KDE Plasma on Wayland (`kwin_wayland`). `was` talks to KWin's
  `org.kde.krunner1` interface at `org.kde.KWin` `/WindowsRunner` — the
  same one KRunner's built-in "windows" plugin uses. No other compositor
  is supported yet (see Architecture below).
```

with:

```markdown
- KDE Plasma or GNOME Shell, both on Wayland. On KDE, `was` talks to
  KWin's `org.kde.krunner1` interface at `org.kde.KWin`
  `/WindowsRunner` — the same one KRunner's built-in "windows" plugin
  uses. On GNOME, `was` requires the
  [Window Calls](https://extensions.gnome.org/extension/4724/window-calls/)
  GNOME Shell extension, since GNOME has no built-in D-Bus interface
  that both lists and activates windows. No other compositor is
  supported yet (see Architecture below).
```

In the `## How it works` section, replace:

```markdown
`was` has a `WindowBackend` trait so window listing/activation isn't
hardwired to one compositor. Right now only `KWinBackend` is
implemented, over KWin's `org.kde.krunner1` D-Bus interface:

- `Match(query)` lists/filters windows (empty query = all windows).
- `Run(id, "")` activates a window by id.

Calls are wrapped in a 3-second timeout so a wedged compositor produces a
clear error instead of hanging the CLI forever.
```

with:

```markdown
`was` has a `WindowBackend` trait so window listing/activation isn't
hardwired to one compositor; `detect_backend()` picks an implementation
based on `XDG_CURRENT_DESKTOP`.

**`KWinBackend`** (KDE) talks to KWin's `org.kde.krunner1` D-Bus
interface:
- `Match(query)` lists/filters windows server-side (empty query = all
  windows).
- `Run(id, "")` activates a window by id.

**`GnomeBackend`** (GNOME) talks to the
[Window Calls](https://extensions.gnome.org/extension/4724/window-calls/)
extension's `org.gnome.Shell.Extensions.Windows` D-Bus interface:
- `List()` returns every window (as JSON); `was` filters client-side
  with a case-insensitive substring match on title/app class, since
  this interface has no server-side query.
- `Activate(id)` activates a window by id.

Both backends wrap their D-Bus calls in a 3-second timeout (shared
helper in `src/timeout.rs`) so a wedged compositor, or a missing/
disabled Window Calls extension on GNOME, produces a clear error
instead of hanging the CLI forever.
```

- [ ] **Step 2: Mark the design spec's JSON-schema assumption as verified**

In `docs/superpowers/specs/2026-07-20-gnome-support-design.md`, find the
paragraph starting "**Assumption flagged for implementation-time
verification:**" and replace it with:

```markdown
**Verified during implementation** (2026-07-20): `List()` returns a
JSON string (D-Bus `s` out-arg, not a native array) containing objects
with `id` (u32), `title`, `wm_class`, `workspace` (i32, among other
unused fields). `Activate(winid: u32)` takes a plain `u` in-arg and
returns nothing. Confirmed against the extension's inlined D-Bus
interface XML and `List()`/`Activate()` implementations in
`extension.js` at
[github.com/ickyicky/window-calls](https://github.com/ickyicky/window-calls).
```

- [ ] **Step 3: Run the full test suite one final time**

Run: `cargo test`
Expected: all tests pass (existing suite + 6 gnome tests + 5 backend-dispatch tests).

- [ ] **Step 4: Release build**

Run: `cargo build --release`
Expected: builds cleanly.

- [ ] **Step 5: Commit**

```bash
git add README.md docs/superpowers/specs/2026-07-20-gnome-support-design.md
git commit -m "Document GNOME support in README and mark design spec assumption verified"
```

---

## Plan Self-Review Notes

- **Spec coverage:** `GnomeBackend` implementing `WindowBackend` over Window Calls (Tasks 1-2); client-side substring matching on title/wm_class, empty-query-means-all (Task 1); `WindowInfo` mapping incl. workspace-based subtext, empty icon (Task 1); missing-extension error naming Window Calls with an install link (Task 2); `detect_backend()` dispatch by `XDG_CURRENT_DESKTOP`, unsupported-desktop error (Task 3); README/spec doc updates (Task 4); explicit acknowledgment that `GnomeBackend`'s real D-Bus calls are unverified in this environment (Global Constraints, Task 2). All spec sections have a task.
- **Type consistency:** `GnomeWindow { id: u32, title: String, wm_class: String, workspace: i32 }` defined once in Task 1, used identically by `matches_query`/`window_info_from_gnome_window` (Task 1) and `GnomeBackend::list_windows` (Task 2). `Backend` enum (`Kde`/`Gnome`) and `backend_for_desktop(&str) -> Result<Backend, String>` defined in Task 3 match exactly how `detect_backend()` consumes them in the same task.
- **Placeholder scan:** no TBD/TODO; Task 2 Step 2's note about possibly adjusting the `&()` call signature is an explicit, named contingency with a clear resolution path (check `kwin.rs`'s existing convention), not a vague placeholder.
