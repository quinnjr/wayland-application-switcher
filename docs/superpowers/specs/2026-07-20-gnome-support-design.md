# GNOME Support — Design

## Purpose

Add a second `WindowBackend` implementation, `GnomeBackend`, so `was`
works on GNOME Shell (Mutter, Wayland) in addition to KDE Plasma
(KWin). `detect_backend()` picks the right one at runtime instead of
always constructing `KWinBackend`.

## Background / feasibility

GNOME Shell has no built-in D-Bus interface that both lists and
activates arbitrary windows:

- `org.gnome.Shell.Introspect` (`GetWindows`) is read-only, and window
  title access is gated by a privacy/usage-monitoring permission on
  stock GNOME — not reliably usable, and has no activation method at
  all.
- `org.gnome.Shell.Eval` can do anything (including activation) via
  arbitrary JS, but is disabled by default since ~GNOME 3.34 outside an
  interactively-enabled "unsafe mode" — not viable as an unattended
  mechanism.
- GNOME Shell's `SearchProvider2` interface is for search providers
  registering *into* the shell (the reverse direction of what's
  needed) — there's no built-in "windows" search provider analogous to
  KWin's `WindowsRunner` that `was` could query.

The only practical path is requiring the user install the **Window
Calls** GNOME Shell extension
([extensions.gnome.org/extension/4724](https://extensions.gnome.org/extension/4724/window-calls/)),
which exposes `org.gnome.Shell.Extensions.Windows` at
`/org/gnome/Shell/Extensions/Windows` with (at minimum) `List()` and
`Activate(id)`. This mirrors the KWin integration's shape — one D-Bus
interface, list + activate — just gated behind a one-time extension
install rather than being built into the compositor.

**Assumption flagged for implementation-time verification:** the exact
JSON field names `List()` returns (`wm_class`, `title`, `workspace`,
etc.) are based on external research, not a live GNOME session in this
environment (this dev machine is KDE Plasma). Confirm the real schema
against the extension's README/source
([github.com/ickyicky/window-calls](https://github.com/ickyicky/window-calls))
during implementation before finalizing the deserialization type.

## Scope

- New `GnomeBackend` implementing the existing `WindowBackend` trait —
  no changes to the trait itself.
- `detect_backend()` changes from "always `KWinBackend`" to "pick by
  `XDG_CURRENT_DESKTOP`".
- Not in scope: any other compositor, a fallback path for GNOME
  without the Window Calls extension (switch/notify simply won't work
  there — see Error handling), automated integration testing against a
  real or mocked GNOME session (this environment can't run one; see
  Testing).

## Architecture

`detect_backend()` (`src/backend/mod.rs`) reads `XDG_CURRENT_DESKTOP`
and dispatches:

```rust
pub fn detect_backend() -> anyhow::Result<Box<dyn WindowBackend>> {
    let desktop = std::env::var("XDG_CURRENT_DESKTOP").unwrap_or_default();
    if desktop.contains("GNOME") {
        Ok(Box::new(gnome::GnomeBackend::connect()?))
    } else if desktop.contains("KDE") {
        Ok(Box::new(kwin::KWinBackend::connect()?))
    } else {
        anyhow::bail!(
            "unsupported desktop environment (XDG_CURRENT_DESKTOP={desktop:?}); \
             only KDE Plasma and GNOME are supported"
        )
    }
}
```

`GnomeBackend` (`src/backend/gnome.rs`) mirrors `KWinBackend`'s shape:

```rust
const DEST: &str = "org.gnome.Shell";
const PATH: &str = "/org/gnome/Shell/Extensions/Windows";
const IFACE: &str = "org.gnome.Shell.Extensions.Windows";

pub struct GnomeBackend {
    conn: Connection,
}

impl GnomeBackend {
    pub fn connect() -> anyhow::Result<Self> {
        Ok(Self { conn: Connection::session()? })
    }
}

impl WindowBackend for GnomeBackend {
    fn list_windows(&self, query: &str) -> anyhow::Result<Vec<WindowInfo>> { ... }
    fn activate(&self, id: &str) -> anyhow::Result<()> { ... }
}
```

Both methods route their D-Bus call through the shared
`crate::timeout::run_with_timeout`/error-mapping pattern already used
by `KWinBackend` (`src/backend/kwin.rs`'s `call_with_timeout`), so a
wedged GNOME Shell or a missing Window Calls extension produces the
same class of clear, bounded error `KWinBackend` already gives for
KWin — reusing the existing helper rather than duplicating it.

## Matching semantics

`List()` takes no query — it always returns every window. Filtering
happens client-side in `GnomeBackend::list_windows`:

- Deserialize the D-Bus reply into `Vec<GnomeWindow>` (a private struct
  matching the extension's JSON shape: `id`, `title`, `wm_class`,
  `workspace`, plus whatever else the reply carries that's unused).
- Empty query → keep every window (matches KWin's "empty query = all
  windows" convention, and the trait's documented contract).
- Non-empty query → keep a window if the query is a case-insensitive
  substring of its `title` OR its `wm_class`.
- Map each surviving `GnomeWindow` to `WindowInfo`:
  - `id` ← `GnomeWindow.id`, stringified.
  - `title` ← `GnomeWindow.title`.
  - `icon` ← `String::new()` (Window Calls doesn't expose an icon;
    consistent with `icon` being unused elsewhere in `was` today).
  - `subtext` ← `format!("Activate running window on workspace {}", GnomeWindow.workspace)`,
    mirroring KWin's `"Activate running window on Desktop {n}"` phrasing
    so the picker reads consistently across backends.

`activate(id)` parses `id` back to whatever type `Activate()` expects
(likely a numeric window id) and calls `Activate(id)`.

## Error handling

- Window Calls not installed/enabled: the `Activate`/`List` D-Bus call
  fails; wrapped into an error naming the extension and pointing at
  `extensions.gnome.org/extension/4724` to install it (GNOME's
  equivalent of KWin's "not reachable"/"is not responding" errors).
- Unsupported desktop (`XDG_CURRENT_DESKTOP` matches neither KDE nor
  GNOME): clear error naming the detected value, exit 1, no panic.
- A GNOME session with an incompatible/older Window Calls API version
  producing a deserialization error: surfaces as a plain D-Bus/serde
  error via the existing `?` propagation — no special handling beyond
  what already exists, consistent with how KWin errors propagate.

## Testing

- Unit tests cover the pure filtering/mapping logic — a
  `fn window_info_from_gnome_window(w: GnomeWindow) -> WindowInfo` and
  the substring-match filter — against fake `GnomeWindow` fixtures:
  empty query (all kept), title match, wm_class match, no match,
  case-insensitivity. Mirrors how `KWinBackend::window_info_from_match`
  is tested today.
- `detect_backend()`'s `XDG_CURRENT_DESKTOP` dispatch logic is
  extracted into a small pure function (e.g.
  `fn backend_for_desktop(desktop: &str) -> Result<Backend, String>`
  returning an enum tag rather than constructing a real backend) so
  the KDE/GNOME/unsupported branching is unit-testable without a live
  D-Bus connection.
- `GnomeBackend`'s real D-Bus calls are **not** exercised in this
  session — this dev environment is KDE Plasma, with no GNOME/Mutter
  session available to test against. Manual verification must happen
  on a real GNOME box (or VM) with the Window Calls extension
  installed, at a later time, the same way `KWinBackend` was
  originally verified live here.
