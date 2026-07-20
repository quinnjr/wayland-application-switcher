# Wayland Application Switcher Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build `was`, a CLI tool that lists/activates KWin windows over D-Bus, with a picker TUI for ambiguous `switch`/`list` matches and a `notify` subcommand that raises a window when its own notification is clicked.

**Architecture:** A `WindowBackend` trait abstracts window listing/activation; `KWinBackend` implements it over `org.kde.KWin` `/WindowsRunner`'s `org.kde.krunner1` D-Bus interface (`Match`/`Run`). A pure `resolve()` function (0/1/many-match branching) is shared by `switch` and the picker; `notify` does *not* use `resolve()` or the picker — on ambiguous match it reports candidates and exits without activating anything, since a notification click has no terminal to render a picker into. `resolve()` is unit-tested against a fake backend and fake picker; the picker's own interaction logic (selection/activate/cancel) is unit-tested headlessly via `ntui::testing::TestTerminal`. `KWinBackend` is verified manually against the live KDE session, since there's no way to unit-test real D-Bus interaction.

**Tech Stack:** Rust 2024 edition, `clap` (derive) for CLI, `zbus` (`async-io` feature, pulling in `blocking`) for D-Bus, `ntui` (Ink-style TUI, hooks-based) for the picker, `tokio` (`rt`, `macros`) as the runtime `ntui` needs, `anyhow` for error handling.

> **Note:** Task 1 below still shows the plan's original `eframe`/`egui` dependency choice — that was superseded by `ntui` partway through implementation (see Task 4, rewritten below to match what was actually built) after the user asked for a TUI instead of a GUI. `notify`'s ambiguous-match handling (Task 7) was correspondingly changed to report-and-skip rather than open a picker, since a notification click has no terminal to render one into.

## Global Constraints

- Binary name is `was` (set via `[[bin]]` in Cargo.toml); the package/crate name stays `wayland-application-switcher`.
- No persistent background daemon — `notify` is a one-shot blocking process (spec: Notify flow).
- Only `KWinBackend` is implemented; the trait exists for future backends but nothing else is built now (spec: Scope).
- Empty query to `list`/`switch` means "all windows", non-empty query is passed through to KRunner's own matching (spec: Matching semantics).
- Picker cancellation is exit code 0, not an error (spec: Error handling).
- Zero matches is exit code 1 with a message naming the query (spec: Error handling).

---

## File Structure

- `Cargo.toml` — package `wayland-application-switcher`, bin `was`, dependencies.
- `src/main.rs` — clap CLI definition, dispatch to command functions, maps `anyhow::Result` to process exit code.
- `src/backend/mod.rs` — `WindowBackend` trait, `WindowInfo` struct, `detect_backend()`.
- `src/backend/kwin.rs` — `KWinBackend`, real D-Bus calls to `org.kde.KWin` `/WindowsRunner`.
- `src/picker.rs` — `Picker` trait, `GuiPicker` (eframe/egui implementation).
- `src/resolve.rs` — pure `resolve()` function (match-count branching) shared by `switch` and `notify`; this is the fully unit-tested core.
- `src/commands/mod.rs` — re-exports `switch`, `list`, `notify` command functions.
- `src/commands/switch.rs` — `run_switch()`.
- `src/commands/list.rs` — `run_list()`.
- `src/commands/notify.rs` — `run_notify()`, notification send + blocking wait + `should_activate()` pure decision function.

---

## Task 1: Project scaffold and dependencies

**Files:**
- Modify: `Cargo.toml`
- Modify: `src/main.rs`

**Interfaces:**
- Produces: a compiling binary named `was` that prints `-h` usage (no subcommands wired yet).

- [ ] **Step 1: Write `Cargo.toml`**

```toml
[package]
name = "wayland-application-switcher"
version = "0.1.0"
edition = "2024"

[[bin]]
name = "was"
path = "src/main.rs"

[dependencies]
clap = { version = "4", features = ["derive"] }
zbus = { version = "4", default-features = false, features = ["blocking"] }
eframe = "0.29"
egui = "0.29"
anyhow = "1"
```

- [ ] **Step 2: Write a stub `src/main.rs`**

```rust
fn main() {
    println!("was: not yet implemented");
}
```

- [ ] **Step 3: Build to confirm dependencies resolve**

Run: `cargo build`
Expected: builds successfully (network access to crates.io required; if `eframe`/`egui` pull in a lot of transitive deps that's expected — this is a GUI toolkit).

- [ ] **Step 4: Commit**

```bash
git add Cargo.toml Cargo.lock src/main.rs
git commit -m "Scaffold was binary with core dependencies"
```

---

## Task 2: WindowBackend trait, WindowInfo, and pure resolve() logic (TDD core)

**Files:**
- Create: `src/backend/mod.rs`
- Create: `src/picker.rs` (only the `Picker` trait in this task — no GUI yet)
- Create: `src/resolve.rs`
- Test: inline `#[cfg(test)]` modules in `src/resolve.rs`

**Interfaces:**
- Produces:
  ```rust
  // src/backend/mod.rs
  pub struct WindowInfo {
      pub id: String,
      pub title: String,
      pub icon: String,
      pub subtext: String,
  }

  pub trait WindowBackend {
      fn list_windows(&self, query: &str) -> anyhow::Result<Vec<WindowInfo>>;
      fn activate(&self, id: &str) -> anyhow::Result<()>;
  }

  // src/picker.rs
  pub trait Picker {
      fn pick(&self, windows: &[WindowInfo]) -> Option<String>;
  }

  // src/resolve.rs
  pub fn resolve(
      backend: &dyn WindowBackend,
      picker: &dyn Picker,
      query: &str,
  ) -> anyhow::Result<()>;
  ```
- Consumes: nothing (this is the foundational task).

- [ ] **Step 1: Write `src/backend/mod.rs` (trait + struct, no impl yet)**

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WindowInfo {
    pub id: String,
    pub title: String,
    pub icon: String,
    pub subtext: String,
}

pub trait WindowBackend {
    fn list_windows(&self, query: &str) -> anyhow::Result<Vec<WindowInfo>>;
    fn activate(&self, id: &str) -> anyhow::Result<()>;
}
```

- [ ] **Step 2: Write `src/picker.rs` (trait only)**

```rust
use crate::backend::WindowInfo;

pub trait Picker {
    fn pick(&self, windows: &[WindowInfo]) -> Option<String>;
}
```

- [ ] **Step 3: Write the failing tests for `resolve()` in `src/resolve.rs`**

```rust
use crate::backend::{WindowBackend, WindowInfo};
use crate::picker::Picker;
use std::cell::RefCell;

pub fn resolve(
    backend: &dyn WindowBackend,
    picker: &dyn Picker,
    query: &str,
) -> anyhow::Result<()> {
    unimplemented!()
}

#[cfg(test)]
mod tests {
    use super::*;

    struct FakeBackend {
        windows: Vec<WindowInfo>,
        activated: RefCell<Vec<String>>,
    }

    impl WindowBackend for FakeBackend {
        fn list_windows(&self, _query: &str) -> anyhow::Result<Vec<WindowInfo>> {
            Ok(self.windows.clone())
        }
        fn activate(&self, id: &str) -> anyhow::Result<()> {
            self.activated.borrow_mut().push(id.to_string());
            Ok(())
        }
    }

    fn window(id: &str) -> WindowInfo {
        WindowInfo {
            id: id.to_string(),
            title: format!("Window {id}"),
            icon: String::new(),
            subtext: String::new(),
        }
    }

    struct FakePicker(Option<String>);

    impl Picker for FakePicker {
        fn pick(&self, _windows: &[WindowInfo]) -> Option<String> {
            self.0.clone()
        }
    }

    #[test]
    fn zero_matches_errors() {
        let backend = FakeBackend { windows: vec![], activated: RefCell::new(vec![]) };
        let picker = FakePicker(None);
        let err = resolve(&backend, &picker, "nothing").unwrap_err();
        assert!(err.to_string().contains("nothing"));
        assert!(backend.activated.borrow().is_empty());
    }

    #[test]
    fn single_match_activates_without_picker() {
        let backend = FakeBackend {
            windows: vec![window("1")],
            activated: RefCell::new(vec![]),
        };
        let picker = FakePicker(Some("should-not-be-used".to_string()));
        resolve(&backend, &picker, "app").unwrap();
        assert_eq!(*backend.activated.borrow(), vec!["1".to_string()]);
    }

    #[test]
    fn multiple_matches_use_picker_selection() {
        let backend = FakeBackend {
            windows: vec![window("1"), window("2")],
            activated: RefCell::new(vec![]),
        };
        let picker = FakePicker(Some("2".to_string()));
        resolve(&backend, &picker, "app").unwrap();
        assert_eq!(*backend.activated.borrow(), vec!["2".to_string()]);
    }

    #[test]
    fn cancelled_picker_activates_nothing_and_is_not_an_error() {
        let backend = FakeBackend {
            windows: vec![window("1"), window("2")],
            activated: RefCell::new(vec![]),
        };
        let picker = FakePicker(None);
        resolve(&backend, &picker, "app").unwrap();
        assert!(backend.activated.borrow().is_empty());
    }
}
```

- [ ] **Step 4: Wire modules into `src/main.rs` and run tests to verify they fail**

Add to `src/main.rs` (temporarily, above `fn main`):

```rust
mod backend;
mod picker;
mod resolve;
```

Run: `cargo test`
Expected: `zero_matches_errors`, `single_match_activates_without_picker`, and `multiple_matches_use_picker_selection` FAIL (or panic on `unimplemented!()`); `cancelled_picker_activates_nothing_and_is_not_an_error` also panics.

- [ ] **Step 5: Implement `resolve()`**

Replace the `unimplemented!()` body in `src/resolve.rs`:

```rust
pub fn resolve(
    backend: &dyn WindowBackend,
    picker: &dyn Picker,
    query: &str,
) -> anyhow::Result<()> {
    let matches = backend.list_windows(query)?;
    match matches.len() {
        0 => anyhow::bail!("no windows matched \"{query}\""),
        1 => backend.activate(&matches[0].id),
        _ => match picker.pick(&matches) {
            Some(id) => backend.activate(&id),
            None => Ok(()),
        },
    }
}
```

- [ ] **Step 6: Run tests to verify they pass**

Run: `cargo test`
Expected: all 4 tests in `resolve::tests` PASS.

- [ ] **Step 7: Commit**

```bash
git add src/backend/mod.rs src/picker.rs src/resolve.rs src/main.rs
git commit -m "Add WindowBackend/Picker traits and tested resolve() core logic"
```

---

## Task 3: KWinBackend over org.kde.krunner1

**Files:**
- Create: `src/backend/kwin.rs`
- Modify: `src/backend/mod.rs` (add `pub mod kwin;` and `detect_backend()`)

**Interfaces:**
- Consumes: `WindowBackend` trait and `WindowInfo` struct from Task 2.
- Produces:
  ```rust
  // src/backend/kwin.rs
  pub struct KWinBackend { /* holds a zbus::blocking::Connection */ }
  impl KWinBackend {
      pub fn connect() -> anyhow::Result<Self>;
  }
  impl WindowBackend for KWinBackend { /* list_windows, activate */ }

  // src/backend/mod.rs
  pub fn detect_backend() -> anyhow::Result<Box<dyn WindowBackend>>;
  ```

This task has no automated tests (real D-Bus/KWin required); it is verified
manually against the live session, matching the spec's testing section.

- [ ] **Step 1: Write `src/backend/kwin.rs`**

```rust
use crate::backend::{WindowBackend, WindowInfo};
use std::collections::HashMap;
use zbus::blocking::Connection;
use zbus::zvariant::OwnedValue;

const DEST: &str = "org.kde.KWin";
const PATH: &str = "/WindowsRunner";
const IFACE: &str = "org.kde.krunner1";

pub struct KWinBackend {
    conn: Connection,
}

impl KWinBackend {
    pub fn connect() -> anyhow::Result<Self> {
        let conn = Connection::session()?;
        // Fail fast with a clear error if org.kde.KWin isn't on the bus.
        conn.call_method(
            Some(DEST),
            "/",
            Some("org.freedesktop.DBus.Peer"),
            "Ping",
            &(),
        )
        .map_err(|e| anyhow::anyhow!("KWin not reachable on session bus: {e}"))?;
        Ok(Self { conn })
    }
}

type MatchTuple = (String, String, String, i32, f64, HashMap<String, OwnedValue>);

impl WindowBackend for KWinBackend {
    fn list_windows(&self, query: &str) -> anyhow::Result<Vec<WindowInfo>> {
        let reply = self.conn.call_method(
            Some(DEST),
            PATH,
            Some(IFACE),
            "Match",
            &(query,),
        )?;
        let matches: Vec<MatchTuple> = reply.body().deserialize()?;
        Ok(matches
            .into_iter()
            .map(|(id, title, icon, _category, _relevance, properties)| {
                let subtext = properties
                    .get("subtext")
                    .and_then(|v| v.downcast_ref::<zbus::zvariant::Str>().ok())
                    .map(|s| s.to_string())
                    .unwrap_or_default();
                WindowInfo { id, title, icon, subtext }
            })
            .collect())
    }

    fn activate(&self, id: &str) -> anyhow::Result<()> {
        self.conn.call_method(
            Some(DEST),
            PATH,
            Some(IFACE),
            "Run",
            &(id, ""),
        )?;
        Ok(())
    }
}
```

- [ ] **Step 2: Add `detect_backend()` to `src/backend/mod.rs`**

```rust
pub mod kwin;

pub fn detect_backend() -> anyhow::Result<Box<dyn WindowBackend>> {
    Ok(Box::new(kwin::KWinBackend::connect()?))
}
```

- [ ] **Step 3: Build**

Run: `cargo build`
Expected: compiles. If `OwnedValue::downcast_ref` / `zvariant::Str` names don't match the installed zbus 4.x API, adjust to whatever that version exposes for reading a string out of a `Value` (check with `cargo doc --open -p zbus` or `cargo tree | grep zbus` to confirm the resolved version first).

- [ ] **Step 4: Manual smoke test against the live KWin session**

Temporarily add to `src/main.rs`, above `fn main`, a throwaway `#[test]`-free manual check — or just test from a scratch binary. Simplest: add a hidden debug path in `main()` for now (removed in Task 5 once the real CLI exists):

```rust
fn main() {
    let backend = backend::detect_backend().expect("backend detect failed");
    let windows = backend.list_windows("konsole").expect("list_windows failed");
    for w in &windows {
        println!("{:?}", w);
    }
}
```

Run: `cargo run` with a Konsole window open.
Expected: prints at least one `WindowInfo` with a non-empty `title` containing "Konsole".

- [ ] **Step 5: Commit**

```bash
git add src/backend/kwin.rs src/backend/mod.rs src/main.rs
git commit -m "Add KWinBackend over org.kde.krunner1 WindowsRunner"
```

---

## Task 4: TuiPicker (ntui) — as actually implemented

> Superseded from an original `eframe`/`egui` GUI design to an `ntui` TUI at
> the user's explicit request, mid-implementation. This section reflects
> what was actually built and committed (commit: "Add TuiPicker using ntui,
> with headless TestTerminal interaction tests").

**Files:**
- Modify: `src/picker.rs` (add `TuiPicker`, `PickerView`, `PickerViewProps`, `ResultSlot` alongside the existing `Picker` trait)
- Modify: `Cargo.toml` (replace `eframe`/`egui` with `ntui` and `tokio`)

**Interfaces:**
- Consumes: `Picker` trait and `WindowInfo` from Task 2.
- Produces: `pub struct TuiPicker;` implementing `Picker` — `impl Picker for TuiPicker { fn pick(&self, windows: &[WindowInfo]) -> Option<String> }`.

`ntui` (`https://github.com/quinnjr/ntui`) is an Ink-style, hooks-based TUI
library over `crossterm`, requiring a `tokio` runtime. Components are
`#[component] fn Name(props: &NameProps, hooks: &mut Hooks) -> Element`;
`NameProps` must be `Clone + PartialEq + Default`. `render(Element) -> impl
Future<Output = Result<(), Error>>` drives a component tree against a real
terminal until `hooks.use_app().exit()` is called; `ntui::testing::TestTerminal`
does the same headlessly, frame by frame, for tests — no real terminal or
display needed.

Getting the picked window id out of `render()` (which only returns
`Result<(), Error>`, no app data) requires a props field that's a shared
cell: `ResultSlot(Arc<Mutex<Option<String>>>)`, with a hand-written
`PartialEq` (pointer identity via `Arc::ptr_eq`) since `Mutex` itself isn't
`PartialEq` — the props-diffing machinery needs *some* `PartialEq` impl to
exist, and identity is the only sensible one for a mutable cell used as an
out-parameter.

- [ ] **Step 1: `zbus`'s "blocking" feature alone is insufficient — fix that first**

Before writing the picker: Task 1/3's `zbus = { version = "4", default-features = false, features = ["blocking"] }` was actually wrong (it happened to build then, but broke once `tokio` was added as a direct dependency and Cargo's feature unification changed). In zbus 4.4, the `blocking` feature flag alone doesn't pull in the async-io executor (`async-io`/`async-lock`/`async-fs`) that the blocking API is implemented on top of — only the `async-io` feature does (and it implies `blocking`). Fix `Cargo.toml`:

```toml
zbus = { version = "4", default-features = false, features = ["async-io"] }
```

- [ ] **Step 2: Add `ntui` and `tokio` to `Cargo.toml`, remove `eframe`/`egui`**

```toml
[dependencies]
clap = { version = "4", features = ["derive"] }
zbus = { version = "4", default-features = false, features = ["async-io"] }
ntui = "0.1"
tokio = { version = "1", features = ["rt", "macros"] }
anyhow = "1"
```

(`macros` is needed for `#[tokio::test]` in this task's own tests, not just for production code.)

- [ ] **Step 3: Implement `TuiPicker` in `src/picker.rs`**

```rust
use crate::backend::WindowInfo;
use ntui::{component, element, render, BorderStyle, Color, Element, FlexDirection, KeyCode, Weight};
use std::sync::{Arc, Mutex};

pub trait Picker {
    fn pick(&self, windows: &[WindowInfo]) -> Option<String>;
}

pub struct TuiPicker;

#[derive(Clone, Default)]
struct ResultSlot(Arc<Mutex<Option<String>>>);

impl PartialEq for ResultSlot {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}

#[derive(Clone, PartialEq, Default)]
struct PickerViewProps {
    windows: Vec<WindowInfo>,
    result: ResultSlot,
}

#[component]
fn PickerView(props: &PickerViewProps, hooks: &mut ntui::Hooks) -> ntui::Element {
    let selected = hooks.use_state(|| 0usize);
    let app = hooks.use_app();

    let windows = props.windows.clone();
    let result = props.result.clone();
    let sel = selected.clone();
    hooks.use_input(move |ev, _| {
        let len = windows.len();
        match ev.code {
            KeyCode::Down => {
                let i = sel.get();
                sel.set((i + 1).min(len.saturating_sub(1)));
            }
            KeyCode::Up => {
                let i = sel.get();
                sel.set(i.saturating_sub(1));
            }
            KeyCode::Enter => {
                let i = sel.get();
                *result.0.lock().unwrap_or_else(|e| e.into_inner()) =
                    windows.get(i).map(|w| w.id.clone());
                app.exit();
            }
            KeyCode::Esc => {
                *result.0.lock().unwrap_or_else(|e| e.into_inner()) = None;
                app.exit();
            }
            _ => {}
        }
    });

    let idx = selected.get();
    element! {
        View(flex_direction: FlexDirection::Column, padding: 1, border_style: BorderStyle::Single) {
            Text(content: "was — pick a window (up/down + enter, esc to cancel)", color: Color::DarkGrey)
            #(props.windows.iter().enumerate().map(|(i, w)| {
                let marker = if i == idx { ">" } else { " " };
                element! {
                    Text(content: format!("{marker} {}  {}", w.title, w.subtext),
                         weight: if i == idx { Weight::Bold } else { Weight::Normal },
                         key: w.id.clone())
                }
            }))
        }
    }
}

impl Picker for TuiPicker {
    fn pick(&self, windows: &[WindowInfo]) -> Option<String> {
        let result = ResultSlot::default();
        let props = PickerViewProps {
            windows: windows.to_vec(),
            result: result.clone(),
        };
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("failed to start ntui runtime");
        rt.block_on(render(Element::component::<PickerView>(props)))
            .expect("ntui render failed");
        result.0.lock().unwrap_or_else(|e| e.into_inner()).clone()
    }
}
```

- [ ] **Step 4: Write headless interaction tests using `ntui::testing::TestTerminal`**

Append to `src/picker.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use ntui::testing::TestTerminal;
    use ntui::KeyCode;

    fn window(id: &str, title: &str) -> WindowInfo {
        WindowInfo {
            id: id.to_string(),
            title: title.to_string(),
            icon: String::new(),
            subtext: "Desktop 1".to_string(),
        }
    }

    fn harness(windows: Vec<WindowInfo>) -> (TestTerminal, ResultSlot) {
        let result = ResultSlot::default();
        let props = PickerViewProps { windows, result: result.clone() };
        let terminal = TestTerminal::new(60, 10, Element::component::<PickerView>(props)).unwrap();
        (terminal, result)
    }

    #[tokio::test]
    async fn initial_frame_selects_first_window() {
        let (t, _result) = harness(vec![window("1", "Firefox"), window("2", "Konsole")]);
        let frame = t.frame_text();
        assert!(frame.contains("> Firefox"));
        assert!(!frame.contains("> Konsole"));
    }

    #[tokio::test]
    async fn down_moves_selection_to_next_window() {
        let (mut t, _result) = harness(vec![window("1", "Firefox"), window("2", "Konsole")]);
        t.send_key(KeyCode::Down).unwrap();
        let frame = t.frame_text();
        assert!(frame.contains("> Konsole"));
        assert!(!frame.contains("> Firefox"));
    }

    #[tokio::test]
    async fn down_does_not_move_past_last_window() {
        let (mut t, _result) = harness(vec![window("1", "Firefox"), window("2", "Konsole")]);
        t.send_key(KeyCode::Down).unwrap();
        t.send_key(KeyCode::Down).unwrap();
        let frame = t.frame_text();
        assert!(frame.contains("> Konsole"));
    }

    #[tokio::test]
    async fn enter_selects_current_window_and_exits() {
        let (mut t, result) = harness(vec![window("1", "Firefox"), window("2", "Konsole")]);
        t.send_key(KeyCode::Down).unwrap();
        t.send_key(KeyCode::Enter).unwrap();
        assert!(t.exited());
        assert_eq!(result.0.lock().unwrap().as_deref(), Some("2"));
    }

    #[tokio::test]
    async fn esc_cancels_and_exits_with_no_result() {
        let (mut t, result) = harness(vec![window("1", "Firefox"), window("2", "Konsole")]);
        t.send_key(KeyCode::Esc).unwrap();
        assert!(t.exited());
        assert_eq!(*result.0.lock().unwrap(), None);
    }
}
```

- [ ] **Step 5: Run tests**

Run: `cargo test`
Expected: 9 tests pass (4 `resolve::tests` from Task 2, 5 new `picker::tests`).

- [ ] **Step 6: Manual visual smoke test (rendering only — see note below on interaction)**

Temporarily call `picker::TuiPicker.pick(&windows)` from `main()` with a couple of fake `WindowInfo` entries and run it inside a real terminal (e.g. `konsole -e ./target/debug/was`, or directly in an interactive shell) — `cargo run` alone inside a non-interactive tool-harness shell will fail with an `Io` error ("No such device or address") since `ntui` needs a real tty for raw-mode/crossterm.

Note: driving *interaction* (arrow keys, Enter) against a real terminal window from an automation harness without a proper input-injection tool (`ydotool`/`wtype`) is unreliable in a pure-Wayland KDE session — `xdotool` cannot target or reliably deliver keys to native Wayland surfaces here. The Step 4 `TestTerminal` tests are the actual verification of interaction correctness; the manual step here only confirms visual rendering in situ.

- [ ] **Step 7: Commit**

```bash
git add src/picker.rs Cargo.toml Cargo.lock src/main.rs .gitignore
git commit -m "Add TuiPicker using ntui, with headless TestTerminal interaction tests"
```

---

## Task 5: CLI wiring — `switch` and `list` subcommands

**Files:**
- Create: `src/commands/mod.rs`
- Create: `src/commands/switch.rs`
- Create: `src/commands/list.rs`
- Modify: `src/main.rs` (replace debug scaffolding with real clap CLI)

**Interfaces:**
- Consumes: `detect_backend()` (Task 3), `TuiPicker` (Task 4), `resolve()` (Task 2).
- Produces: `run_switch(query: &str) -> anyhow::Result<()>`, `run_list(query: Option<&str>) -> anyhow::Result<()>`.

- [ ] **Step 1: Write `src/commands/switch.rs`**

```rust
use crate::backend::detect_backend;
use crate::picker::TuiPicker;
use crate::resolve::resolve;

pub fn run_switch(query: &str) -> anyhow::Result<()> {
    let backend = detect_backend()?;
    let picker = TuiPicker;
    resolve(backend.as_ref(), &picker, query)
}
```

- [ ] **Step 2: Write `src/commands/list.rs`**

```rust
use crate::backend::detect_backend;

pub fn run_list(query: Option<&str>) -> anyhow::Result<()> {
    let backend = detect_backend()?;
    let windows = backend.list_windows(query.unwrap_or(""))?;
    for w in &windows {
        println!("{}\t{}\t{}", w.id, w.title, w.subtext);
    }
    Ok(())
}
```

- [ ] **Step 3: Write `src/commands/mod.rs`**

```rust
pub mod list;
pub mod switch;

pub use list::run_list;
pub use switch::run_switch;
```

- [ ] **Step 4: Replace `src/main.rs` with the real CLI (notify wired in Task 7 — stub it as unimplemented for now)**

```rust
mod backend;
mod commands;
mod picker;
mod resolve;

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "was", about = "Wayland application switcher")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Activate the window matching QUERY (or pick among several matches).
    Switch { query: String },
    /// List windows matching QUERY (all windows if omitted).
    List { query: Option<String> },
}

fn main() -> std::process::ExitCode {
    let cli = Cli::parse();
    let result = match cli.command {
        Command::Switch { query } => commands::run_switch(&query),
        Command::List { query } => commands::run_list(query.as_deref()),
    };
    match result {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("was: {e}");
            std::process::ExitCode::FAILURE
        }
    }
}
```

- [ ] **Step 5: Build and run unit tests**

Run: `cargo build && cargo test`
Expected: builds; the 4 `resolve::tests` still pass (untouched by this task).

- [ ] **Step 6: Manual smoke test**

Run: `cargo run -- list` — expect a tab-separated line per open window.
Run: `cargo run -- switch konsole` with exactly one Konsole window open — expect it to raise/focus immediately with no picker.
Run: `cargo run -- switch ""` (or any query matching 2+ windows) — expect the picker TUI to render in the current terminal; select one and confirm it's activated.
Run: `cargo run -- switch this-matches-nothing-xyz` — expect stderr `was: no windows matched "this-matches-nothing-xyz"` and a non-zero exit code (`echo $?`).

- [ ] **Step 7: Commit**

```bash
git add src/commands src/main.rs
git commit -m "Wire switch and list subcommands into the was CLI"
```

---

## Task 6: notify — pure decision logic (TDD)

**Files:**
- Create: `src/commands/notify.rs`
- Test: inline `#[cfg(test)]` module in `src/commands/notify.rs`

**Interfaces:**
- Produces:
  ```rust
  pub enum NotifyEvent {
      ActionInvoked { id: u32, action_key: String },
      Closed { id: u32 },
  }

  // Returns Some(true) to activate, Some(false) to exit quietly,
  // None if the event is irrelevant to `target_id` (keep waiting).
  pub fn should_activate(target_id: u32, event: NotifyEvent) -> Option<bool>;
  ```
- Consumes: nothing yet (D-Bus plumbing wired in Task 7).

- [ ] **Step 1: Write the failing tests**

```rust
pub enum NotifyEvent {
    ActionInvoked { id: u32, action_key: String },
    Closed { id: u32 },
}

pub fn should_activate(target_id: u32, event: NotifyEvent) -> Option<bool> {
    unimplemented!()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_action_on_target_id_activates() {
        let event = NotifyEvent::ActionInvoked { id: 42, action_key: "default".to_string() };
        assert_eq!(should_activate(42, event), Some(true));
    }

    #[test]
    fn non_default_action_on_target_id_is_ignored() {
        let event = NotifyEvent::ActionInvoked { id: 42, action_key: "other".to_string() };
        assert_eq!(should_activate(42, event), None);
    }

    #[test]
    fn action_on_different_id_is_ignored() {
        let event = NotifyEvent::ActionInvoked { id: 7, action_key: "default".to_string() };
        assert_eq!(should_activate(42, event), None);
    }

    #[test]
    fn closed_on_target_id_exits_quietly() {
        let event = NotifyEvent::Closed { id: 42 };
        assert_eq!(should_activate(42, event), Some(false));
    }

    #[test]
    fn closed_on_different_id_is_ignored() {
        let event = NotifyEvent::Closed { id: 7 };
        assert_eq!(should_activate(42, event), None);
    }
}
```

- [ ] **Step 2: Add `mod notify;` to `src/commands/mod.rs` and run tests to verify failure**

Run: `cargo test should_activate`
Expected: all 5 tests panic on `unimplemented!()`.

- [ ] **Step 3: Implement `should_activate`**

```rust
pub fn should_activate(target_id: u32, event: NotifyEvent) -> Option<bool> {
    match event {
        NotifyEvent::ActionInvoked { id, action_key } if id == target_id && action_key == "default" => {
            Some(true)
        }
        NotifyEvent::Closed { id } if id == target_id => Some(false),
        _ => None,
    }
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test should_activate`
Expected: all 5 tests PASS.

- [ ] **Step 5: Commit**

```bash
git add src/commands/notify.rs src/commands/mod.rs
git commit -m "Add pure should_activate decision logic for notify clicks"
```

---

## Task 7: notify — D-Bus send + blocking wait + CLI wiring

**Files:**
- Modify: `src/commands/notify.rs` (add `run_notify`)
- Modify: `src/commands/mod.rs` (export `run_notify`)
- Modify: `src/main.rs` (add `Notify` subcommand)

**Interfaces:**
- Consumes: `should_activate`/`NotifyEvent` (Task 6), `detect_backend()` (Task 3). Does **not** consume `resolve()` or `TuiPicker` — notify never opens a picker (spec amendment: ambiguous notify matches are reported and left un-activated, not resolved interactively).
- Produces: `pub fn run_notify(query: &str, summary: &str, body: Option<&str>, icon: Option<&str>) -> anyhow::Result<()>`.

No automated tests for the D-Bus plumbing itself (real notification daemon
required); verified manually.

- [ ] **Step 1: Add `run_notify` to `src/commands/notify.rs`**

```rust
use crate::backend::detect_backend;
use std::collections::HashMap;
use zbus::blocking::Connection;
use zbus::zvariant::Value;
use zbus::MatchRule;

pub fn run_notify(
    query: &str,
    summary: &str,
    body: Option<&str>,
    icon: Option<&str>,
) -> anyhow::Result<()> {
    let conn = Connection::session()?;

    let actions: Vec<&str> = vec!["default", ""];
    let hints: HashMap<&str, Value> = HashMap::new();

    let reply = conn.call_method(
        Some("org.freedesktop.Notifications"),
        "/org/freedesktop/Notifications",
        Some("org.freedesktop.Notifications"),
        "Notify",
        &(
            "was",
            0u32,
            icon.unwrap_or(""),
            summary,
            body.unwrap_or(""),
            actions,
            hints,
            -1i32,
        ),
    )?;
    let notification_id: u32 = reply.body().deserialize()?;

    // A single match rule on the interface catches both signals; the loop
    // below tells them apart by checking each message's member name.
    let rule = MatchRule::builder()
        .msg_type(zbus::message::Type::Signal)
        .interface("org.freedesktop.Notifications")?
        .path("/org/freedesktop/Notifications")?
        .build();
    let mut iter = zbus::blocking::MessageIterator::for_match_rule(rule, &conn, Some(16))?;

    let activate = loop {
        let Some(msg) = iter.next() else { break false };
        let msg = msg?;
        let Some(member) = msg.header().member().map(|m| m.to_string()) else { continue };
        let event = match member.as_str() {
            "ActionInvoked" => {
                let (id, action_key): (u32, String) = msg.body().deserialize()?;
                NotifyEvent::ActionInvoked { id, action_key }
            }
            "NotificationClosed" => {
                let (id, _reason): (u32, u32) = msg.body().deserialize()?;
                NotifyEvent::Closed { id }
            }
            _ => continue,
        };
        if let Some(decision) = should_activate(notification_id, event) {
            break decision;
        }
    };

    if !activate {
        return Ok(());
    }

    let backend = detect_backend()?;
    let matches = backend.list_windows(query)?;
    match matches.len() {
        0 => anyhow::bail!("no windows matched \"{query}\""),
        1 => backend.activate(&matches[0].id)?,
        _ => {
            eprintln!("was: \"{query}\" is ambiguous, not activating anything:");
            for w in &matches {
                eprintln!("  {}\t{}", w.id, w.title);
            }
        }
    }
    Ok(())
}
```

- [ ] **Step 2: Add `pub use notify::run_notify;` to `src/commands/mod.rs`**

- [ ] **Step 3: Add the `Notify` subcommand to `src/main.rs`**

```rust
#[derive(Subcommand)]
enum Command {
    Switch { query: String },
    List { query: Option<String> },
    /// Send a notification; clicking it activates the window matching QUERY.
    Notify {
        #[arg(long)]
        query: String,
        #[arg(long)]
        summary: String,
        #[arg(long)]
        body: Option<String>,
        #[arg(long)]
        icon: Option<String>,
    },
}
```

And in `main()`'s match:

```rust
Command::Notify { query, summary, body, icon } => {
    commands::run_notify(&query, &summary, body.as_deref(), icon.as_deref())
}
```

- [ ] **Step 4: Build**

Run: `cargo build`
Expected: compiles.

- [ ] **Step 5: Manual smoke test**

Run: `cargo run -- notify --query konsole --summary "test notification"` with exactly one Konsole window open, in one terminal.
Click the notification body when it appears.
Expected: the process was blocking, then exits after the click, and the Konsole window is raised/focused.

Run again with a query matching 2+ windows and click the notification.
Expected: process exits 0, prints the ambiguous candidates to stderr, activates nothing.

Run again and dismiss the notification without clicking.
Expected: process exits quietly (exit code 0) without activating anything.

- [ ] **Step 6: Commit**

```bash
git add src/commands/notify.rs src/commands/mod.rs src/main.rs
git commit -m "Wire notify subcommand: send notification, wait for click, activate"
```

---

## Task 8: Final cleanup and full end-to-end verification

**Files:**
- Modify: `src/main.rs` (remove any leftover debug scaffolding from Tasks 3/4 if not already removed by Task 5's rewrite)

- [ ] **Step 1: Confirm no debug scaffolding remains**

Run: `grep -n "picked:" src/main.rs; grep -n "backend detect failed" src/main.rs`
Expected: no matches (Task 5 already replaced `main.rs` wholesale, so this should be clean — this step just double-checks).

- [ ] **Step 2: Run the full test suite**

Run: `cargo test`
Expected: all 9 tests (4 in `resolve`, 5 in `commands::notify`) pass.

- [ ] **Step 3: Release build**

Run: `cargo build --release`
Expected: builds cleanly, binary at `target/release/was`.

- [ ] **Step 4: End-to-end manual pass**

With at least two apps open (e.g. Konsole and Firefox):
- `./target/release/was list` — lists all open windows.
- `./target/release/was switch firefox` — raises Firefox with no picker (assuming one Firefox window).
- `./target/release/was switch ""` — opens the picker TUI across all windows; pick one, confirm activation.
- `./target/release/was notify --query firefox --summary "test"` then click it — confirms Firefox is raised after click (single match); repeat with an ambiguous query to confirm it reports candidates and activates nothing.

- [ ] **Step 5: Commit (only if Step 1 found and removed anything)**

```bash
git add src/main.rs
git commit -m "Remove leftover debug scaffolding"
```

---

## Plan Self-Review Notes

- **Spec coverage:** `switch`/`list`/`notify` subcommands (Tasks 5, 7); `WindowBackend` trait + KWin backend over `org.kde.krunner1` (Tasks 2–3); picker TUI with 0/1/many branching and cancel-is-not-an-error (Tasks 2, 4); notify one-shot blocking flow with default-action click detection and no-picker ambiguous-match reporting (Tasks 6–7); error handling for no backend / zero matches / activate failure (Task 3's `connect()` error, `resolve()`'s bail, `activate()`'s `?` propagation); unit tests against fake backend/picker for the branching logic, headless `TestTerminal` tests for the picker's own interaction, and pure event data for notify's decision logic (Tasks 2, 4, 6); binary named `was` (Task 1). All spec sections have a task.
- **Type consistency:** `WindowInfo { id, title, icon, subtext }` is defined once in Task 2 and used identically in Tasks 3, 4, 5, 7. `WindowBackend::list_windows(&self, query: &str)` / `activate(&self, id: &str)` signatures match between the Task 2 trait and the Task 3 `KWinBackend` impl and Task 2's `FakeBackend` test double. `Picker::pick(&self, windows: &[WindowInfo]) -> Option<String>` matches between Task 2's trait, Task 2's `FakePicker`, and Task 4's `TuiPicker`.
- **Placeholder scan:** no TBD/TODO.
- **Amendment note:** Task 4 and downstream references to it were changed from an `eframe`/`egui` GUI to an `ntui` TUI at the user's explicit request, mid-implementation (after Task 3 was already committed). `notify`'s ambiguous-match behavior was correspondingly changed from "open the picker" to "report candidates, activate nothing" per a follow-up user decision, since a notification click has no terminal to render a picker into.
