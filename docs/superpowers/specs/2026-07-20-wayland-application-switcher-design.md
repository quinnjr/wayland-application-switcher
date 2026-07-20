# Wayland Application Switcher — Design

## Purpose

A CLI tool (binary name `was`) that lists and activates open windows on a
Wayland compositor, so a user can bring a running application to the front
either directly from the command line, or by clicking a desktop notification
that was sent through this tool.

Target environment: KDE Plasma on Wayland (`kwin_wayland`), confirmed as the
actual running session. The window-listing/activation logic is behind a
backend trait so other compositors (Hyprland, sway, etc.) can be added later
without changing the CLI, picker, or notify flow.

## Scope

- Switch between windows of one named app, or across all open windows —
  both are just "match query against window list", so one code path covers
  both.
- Not in scope: replacing the system notification daemon, intercepting
  notifications sent by *other* applications, a persistent background
  daemon, non-KWin backends (the trait exists, but only `KWinBackend` is
  implemented).

## Architecture

Single binary `was` (package `wayland-application-switcher`), three
subcommands:

- `was switch <QUERY>` — filter windows by app/title; activate the sole
  match, or open a picker GUI if several match.
- `was list [QUERY]` — print matching windows to stdout (scripting/debug
  aid); with no query, prints all windows.
- `was notify --query <QUERY> --summary <TEXT> [--body <TEXT>] [--icon <NAME>]`
  — send a desktop notification whose click raises the window matched by
  `QUERY`.

### WindowBackend trait

```rust
trait WindowBackend {
    fn list_windows(&self, query: &str) -> Result<Vec<WindowInfo>>;
    fn activate(&self, id: &str) -> Result<()>;
}

struct WindowInfo {
    id: String,
    title: String,
    icon: String,
    subtext: String,
}
```

`KWinBackend` implements this over the `org.kde.krunner1` D-Bus interface
exposed at `org.kde.KWin` `/WindowsRunner` (the same interface KRunner's
built-in "windows" plugin uses):

- `Match(query: &str) -> Vec<(id, text, icon_name, relevance, category, properties)>`
  for listing/filtering.
- `Run(id: &str, action_id: &str)` with an empty `action_id` to activate the
  matched window.

Verified live in this session: `Match("konsole")` correctly returned the
open Konsole window with title, icon name, and a "Activate running window
on Desktop 1" subtext.

Backend selection at startup: check for `org.kde.KWin` on the session bus.
If absent, exit with a clear "no supported window backend found" error
(no other backend is implemented yet).

### Matching semantics

`list_windows(query)` with an empty query returns all open windows,
unfiltered (matches the "all windows" alt-tab use case). A non-empty query
is passed straight through to `Match`, which does KRunner's normal
substring/relevance matching against window titles and app names (matches
the "windows of one app" use case). One code path serves both.

## Picker GUI

- 0 matches: print an error naming the query to stderr, exit 1.
- 1 match: activate immediately, no UI shown.
- 2+ matches: open a small always-on-top `eframe`/`egui` window, one row
  per match (title + subtext, icon best-effort). Arrow keys + Enter to
  activate, mouse click also activates, Escape (or focus loss) cancels.
- This is the same picker for both `switch` and post-click `notify`
  activation — it does not assume a TTY is attached, since notification
  clicks happen with no terminal in the picture.
- Cancelling the picker is a normal outcome (exit 0), not a failure.

## Notify flow

One-shot, blocking process — no persistent daemon:

1. Call `org.freedesktop.Notifications.Notify` with a `"default"` action
   registered (so the notification body itself is clickable) plus the
   given summary/body/icon.
2. Block on the session bus for either `ActionInvoked` (the default
   action = a click) or `NotificationClosed`, matched by this
   notification's id. No timeout — the process waits until the user acts
   on or dismisses the notification.
3. On click: run the same match → single-activate-or-picker logic as
   `was switch <QUERY>`.
4. On dismiss without click: exit quietly (exit 0).

Callers background this themselves, e.g.:

```sh
was notify --query slack --summary "New message" & disown
```

This deliberately does not attempt to intercept notifications from other
apps: the freedesktop notification spec gives no reliable, unprivileged way
to rewrite another app's click target. `was` only handles notifications it
sends itself, where it controls the id-to-query correlation directly in
memory (no cross-process state needed).

## Error handling

- No session bus / no `org.kde.KWin` object: clear stderr message, exit 1,
  no panic.
- `switch`/`notify` with zero matches: stderr message naming the query,
  exit 1.
- `activate` D-Bus call failing (e.g. window closed between match and
  click): stderr error, exit 1, no crash.
- Picker cancelled: exit 0 (see above — not an error).

## Testing

- Unit tests cover query-matching/selection branching (0 / 1 / many
  matches) against a fake in-memory `WindowBackend`, independent of D-Bus.
- `KWinBackend` itself is exercised manually against the live session (the
  only backend available to test here), not via automated tests.

## Dependencies

- `clap` — CLI argument parsing.
- `zbus` (`blocking` feature) — D-Bus calls without pulling in an async
  runtime.
- `eframe` / `egui` — picker GUI.

## Cargo.toml shape

```toml
[package]
name = "wayland-application-switcher"
version = "0.1.0"
edition = "2024"

[[bin]]
name = "was"
path = "src/main.rs"
```
