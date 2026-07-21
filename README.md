# was — Wayland Application Switcher

A small CLI for KDE Plasma on Wayland that lists and activates open
windows over D-Bus, with a terminal picker for ambiguous matches and a
`notify` command that raises a window when its own desktop notification
is clicked.

## Requirements

- KDE Plasma or GNOME Shell, both on Wayland. On KDE, `was` talks to
  KWin's `org.kde.krunner1` interface at `org.kde.KWin`
  `/WindowsRunner` — the same one KRunner's built-in "windows" plugin
  uses. On GNOME, `was` requires the
  [Window Calls](https://extensions.gnome.org/extension/4724/window-calls/)
  GNOME Shell extension, since GNOME has no built-in D-Bus interface
  that both lists and activates windows. No other compositor is
  supported yet (see Architecture below).
- Rust 2024 edition / a recent stable or nightly toolchain to build.

## Install / build

```sh
cargo build --release
# binary at target/release/was
```

Pushing a tag matching `v*` (e.g. `v0.1.0`) builds `.deb` and `.rpm`
packages via CI and attaches them to a GitHub Release. To build them
locally:

```sh
cargo install cargo-deb cargo-generate-rpm
cargo build --release
cargo deb --no-build      # target/debian/*.deb
cargo generate-rpm        # target/generate-rpm/*.rpm
```

## Usage

```sh
was list [QUERY]
```
Print windows matching `QUERY` as `id<TAB>title<TAB>subtext`, one per
line. With no query, lists every open window.

```sh
was switch <QUERY>
```
Activate the window matching `QUERY`. If exactly one window matches, it's
raised immediately. If several match, a terminal picker opens (arrow
keys or Up/Down + Enter to activate, Esc to cancel — cancelling exits 0,
it's not an error). If none match, prints an error and exits 1.

```sh
was notify --query <QUERY> --summary <TEXT> [--body <TEXT>] [--icon <NAME>]
```
Sends a desktop notification and blocks until it's clicked or dismissed
(bounded by a 24h timeout, in practice unbounded). On click, activates
the window matching `QUERY` the same way `switch` would — except if the
match is ambiguous, it prints the candidates to stderr and activates
nothing rather than opening a picker (a notification click has no
terminal to render one into). On dismiss without a click, exits quietly.

Since this blocks, background it from whatever triggers the
notification:

```sh
was notify --query slack --summary "New message" & disown
```

## How it works

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
- `List()` returns every window (as JSON); `was` filters client-side,
  since this interface has no server-side query: every
  whitespace-separated query token must appear (case-insensitively) in
  the window's title or app class.
- `Activate(id)` activates a window by id.

Both backends wrap their D-Bus calls in a 3-second timeout (shared
helper in `src/timeout.rs`) so a wedged compositor, or a missing/
disabled Window Calls extension on GNOME, produces a clear error
instead of hanging the CLI forever.

## Development

```sh
cargo test    # unit tests — match-resolution branching, picker
              # interaction (headless via ntui's TestTerminal),
              # notify's click/dismiss decision logic, the timeout
              # helper's ok/elapsed/panicked paths
cargo build --release
```

`KWinBackend` itself and the notify D-Bus flow are exercised manually
against a live KDE session rather than by automated tests, since they
need a real compositor and notification daemon.

Design spec and implementation plan live under `docs/superpowers/`.
