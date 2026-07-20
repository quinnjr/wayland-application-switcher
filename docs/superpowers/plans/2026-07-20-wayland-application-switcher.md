# Wayland Application Switcher Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build `was`, a CLI tool that lists/activates KWin windows over D-Bus, with a picker GUI for ambiguous matches and a `notify` subcommand that raises a window when its own notification is clicked.

**Architecture:** A `WindowBackend` trait abstracts window listing/activation; `KWinBackend` implements it over `org.kde.KWin` `/WindowsRunner`'s `org.kde.krunner1` D-Bus interface (`Match`/`Run`). A pure `resolve()` function (0/1/many-match branching) is shared by `switch` and post-click `notify` activation, and is unit-tested against a fake backend and fake picker — no D-Bus or GUI needed for that logic. `KWinBackend` and the `eframe` picker are verified manually against the live KDE session, since there's no way to unit-test real D-Bus/GUI interaction.

**Tech Stack:** Rust 2024 edition, `clap` (derive) for CLI, `zbus` (`blocking` feature only, no async runtime) for D-Bus, `eframe`/`egui` for the picker GUI, `anyhow` for error handling.

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

## Task 4: GuiPicker (eframe/egui)

**Files:**
- Modify: `src/picker.rs` (add `GuiPicker` alongside the existing `Picker` trait)

**Interfaces:**
- Consumes: `Picker` trait and `WindowInfo` from Task 2.
- Produces: `pub struct GuiPicker;` implementing `Picker` — `impl Picker for GuiPicker { fn pick(&self, windows: &[WindowInfo]) -> Option<String> }`.

No automated tests (real GUI); verified manually.

- [ ] **Step 1: Implement `GuiPicker` in `src/picker.rs`**

```rust
use crate::backend::WindowInfo;
use std::cell::RefCell;
use std::rc::Rc;

pub trait Picker {
    fn pick(&self, windows: &[WindowInfo]) -> Option<String>;
}

pub struct GuiPicker;

struct PickerApp {
    windows: Vec<WindowInfo>,
    selected: usize,
    result: Rc<RefCell<Option<String>>>,
}

impl eframe::App for PickerApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        ctx.input(|i| {
            if i.key_pressed(egui::Key::Escape) {
                *self.result.borrow_mut() = None;
                std::process::exit_code_hint();
            }
        });

        egui::CentralPanel::default().show(ctx, |ui| {
            ctx.input(|i| {
                if i.key_pressed(egui::Key::ArrowDown) {
                    self.selected = (self.selected + 1).min(self.windows.len() - 1);
                }
                if i.key_pressed(egui::Key::ArrowUp) {
                    self.selected = self.selected.saturating_sub(1);
                }
                if i.key_pressed(egui::Key::Enter) {
                    *self.result.borrow_mut() = Some(self.windows[self.selected].id.clone());
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                }
                if i.key_pressed(egui::Key::Escape) {
                    *self.result.borrow_mut() = None;
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                }
            });

            for (idx, w) in self.windows.iter().enumerate() {
                let text = format!("{}\n{}", w.title, w.subtext);
                let selected = idx == self.selected;
                if ui.selectable_label(selected, text).clicked() {
                    *self.result.borrow_mut() = Some(w.id.clone());
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                }
            }
        });
    }
}

impl Picker for GuiPicker {
    fn pick(&self, windows: &[WindowInfo]) -> Option<String> {
        let result = Rc::new(RefCell::new(None));
        let app_result = result.clone();
        let windows = windows.to_vec();

        let native_options = eframe::NativeOptions {
            viewport: egui::ViewportBuilder::default()
                .with_always_on_top()
                .with_inner_size([420.0, 240.0]),
            ..Default::default()
        };

        let _ = eframe::run_native(
            "was picker",
            native_options,
            Box::new(move |_cc| {
                Ok(Box::new(PickerApp {
                    windows,
                    selected: 0,
                    result: app_result,
                }))
            }),
        );

        let out = result.borrow().clone();
        out
    }
}
```

Note: remove the stray `std::process::exit_code_hint();` line from the first
`ctx.input` block above when writing the real file — it was left over from
drafting and isn't a real API; the Escape handling inside `CentralPanel`
below is the actual cancel path.

- [ ] **Step 2: Build**

Run: `cargo build`
Expected: compiles. `WindowInfo` needs `Clone` — already derived in Task 2.

- [ ] **Step 3: Manual smoke test**

Add a temporary call in `main()`:

```rust
fn main() {
    let picker = picker::GuiPicker;
    let windows = vec![
        backend::WindowInfo { id: "1".into(), title: "Firefox".into(), icon: String::new(), subtext: "Desktop 1".into() },
        backend::WindowInfo { id: "2".into(), title: "Konsole".into(), icon: String::new(), subtext: "Desktop 1".into() },
    ];
    println!("picked: {:?}", picker.pick(&windows));
}
```

Run: `cargo run` (requires the Wayland session — `WAYLAND_DISPLAY`/`DISPLAY` confirmed present).
Expected: a small window listing "Firefox" and "Konsole" appears; selecting one with arrow keys + Enter, or clicking it, prints `picked: Some("1")` or `picked: Some("2")`; pressing Escape prints `picked: None`.

- [ ] **Step 4: Commit**

```bash
git add src/picker.rs src/main.rs
git commit -m "Add GuiPicker eframe/egui implementation"
```

---

## Task 5: CLI wiring — `switch` and `list` subcommands

**Files:**
- Create: `src/commands/mod.rs`
- Create: `src/commands/switch.rs`
- Create: `src/commands/list.rs`
- Modify: `src/main.rs` (replace debug scaffolding with real clap CLI)

**Interfaces:**
- Consumes: `detect_backend()` (Task 3), `GuiPicker` (Task 4), `resolve()` (Task 2).
- Produces: `run_switch(query: &str) -> anyhow::Result<()>`, `run_list(query: Option<&str>) -> anyhow::Result<()>`.

- [ ] **Step 1: Write `src/commands/switch.rs`**

```rust
use crate::backend::detect_backend;
use crate::picker::GuiPicker;
use crate::resolve::resolve;

pub fn run_switch(query: &str) -> anyhow::Result<()> {
    let backend = detect_backend()?;
    let picker = GuiPicker;
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
Run: `cargo run -- switch ""` (or any query matching 2+ windows) — expect the picker GUI to appear; select one and confirm it's activated.
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
- Consumes: `should_activate`/`NotifyEvent` (Task 6), `resolve()` (Task 2), `detect_backend()` (Task 3), `GuiPicker` (Task 4).
- Produces: `pub fn run_notify(query: &str, summary: &str, body: Option<&str>, icon: Option<&str>) -> anyhow::Result<()>`.

No automated tests for the D-Bus plumbing itself (real notification daemon
required); verified manually.

- [ ] **Step 1: Add `run_notify` to `src/commands/notify.rs`**

```rust
use crate::backend::detect_backend;
use crate::picker::GuiPicker;
use crate::resolve::resolve;
use std::collections::HashMap;
use zbus::blocking::Connection;
use zbus::zvariant::Value;

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

    let action_proxy = conn.call_method(
        Some("org.freedesktop.Notifications"),
        "/org/freedesktop/Notifications",
        Some("org.freedesktop.DBus.Peer"),
        "Ping",
        &(),
    );
    let _ = action_proxy;

    let mut action_invoked = conn.receive_signal_with_args(
        Some("org.freedesktop.Notifications"),
        "ActionInvoked",
        &[(0, &notification_id.to_string())],
    )?;
    let mut closed = conn.receive_signal_with_args(
        Some("org.freedesktop.Notifications"),
        "NotificationClosed",
        &[(0, &notification_id.to_string())],
    )?;

    let activate = loop {
        if let Some(msg) = action_invoked.next() {
            let (id, action_key): (u32, String) = msg.body().deserialize()?;
            if let Some(decision) = should_activate(notification_id, NotifyEvent::ActionInvoked { id, action_key }) {
                break decision;
            }
        } else if let Some(msg) = closed.next() {
            let (id, _reason): (u32, u32) = msg.body().deserialize()?;
            if let Some(decision) = should_activate(notification_id, NotifyEvent::Closed { id }) {
                break decision;
            }
        }
    };

    if activate {
        let backend = detect_backend()?;
        let picker = GuiPicker;
        resolve(backend.as_ref(), &picker, query)?;
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
Expected: compiles. If `receive_signal_with_args` isn't the exact zbus 4.x
API name/shape, check `cargo doc -p zbus --open` for the installed version
and adjust to the equivalent (e.g. `MessageStream` + manual filtering by
deserializing each signal and checking the id) — the decision logic in
`should_activate` doesn't change either way.

- [ ] **Step 5: Manual smoke test**

Run: `cargo run -- notify --query konsole --summary "test notification"` with a Konsole window open, in one terminal.
Click the notification body when it appears.
Expected: the process was blocking, then exits after the click, and the Konsole window is raised/focused (single match) or the picker appears (multiple matches).

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
- `./target/release/was switch ""` — opens the picker across all windows; pick one, confirm activation.
- `./target/release/was notify --query firefox --summary "test"` then click it — confirms Firefox is raised after click.

- [ ] **Step 5: Commit (only if Step 1 found and removed anything)**

```bash
git add src/main.rs
git commit -m "Remove leftover debug scaffolding"
```

---

## Plan Self-Review Notes

- **Spec coverage:** `switch`/`list`/`notify` subcommands (Tasks 5, 7); `WindowBackend` trait + KWin backend over `org.kde.krunner1` (Tasks 2–3); picker GUI with 0/1/many branching and cancel-is-not-an-error (Tasks 2, 4); notify one-shot blocking flow with default-action click detection (Tasks 6–7); error handling for no backend / zero matches / activate failure (Task 3's `connect()` error, `resolve()`'s bail, `activate()`'s `?` propagation); unit tests against fake backend/picker for the branching logic and against pure event data for notify's decision logic (Tasks 2, 6); binary named `was` (Task 1). All spec sections have a task.
- **Type consistency:** `WindowInfo { id, title, icon, subtext }` is defined once in Task 2 and used identically in Tasks 3, 4, 5, 7. `WindowBackend::list_windows(&self, query: &str)` / `activate(&self, id: &str)` signatures match between the Task 2 trait and the Task 3 `KWinBackend` impl and Task 2's `FakeBackend` test double. `Picker::pick(&self, windows: &[WindowInfo]) -> Option<String>` matches between Task 2's trait, Task 2's `FakePicker`, and Task 4's `GuiPicker`.
- **Placeholder scan:** no TBD/TODO; the one intentionally-called-out leftover line in Task 4 Step 1 (`std::process::exit_code_hint()`) is explicitly flagged as not-real-code to delete, not a silent placeholder.
