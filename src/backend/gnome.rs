use crate::backend::{WindowBackend, WindowInfo};
use crate::timeout::call_dbus_with_timeout;
use anyhow::Context;
use std::time::Duration;
use zbus::blocking::Connection;

const DEST: &str = "org.gnome.Shell";
const PATH: &str = "/org/gnome/Shell/Extensions/Windows";
const IFACE: &str = "org.gnome.Shell.Extensions.Windows";
const CALL_TIMEOUT: Duration = Duration::from_secs(3);
const INSTALL_HINT: &str = "is the \"Window Calls\" GNOME Shell extension installed and enabled? \
     https://extensions.gnome.org/extension/4724/window-calls/";

pub struct GnomeBackend {
    conn: Connection,
}

impl GnomeBackend {
    pub fn connect() -> anyhow::Result<Self> {
        Ok(Self {
            conn: Connection::session()?,
        })
    }
}

fn call_with_timeout<T: Send + 'static>(
    context: &'static str,
    hint: Option<&'static str>,
    f: impl FnOnce() -> zbus::Result<T> + Send + 'static,
) -> anyhow::Result<T> {
    call_dbus_with_timeout(CALL_TIMEOUT, "GNOME Shell", context, hint, f)
}

impl WindowBackend for GnomeBackend {
    fn list_windows(&self, query: &str) -> anyhow::Result<Vec<WindowInfo>> {
        let conn = self.conn.clone();
        // The install hint only makes sense here: List is the first call any
        // command makes, so a missing extension always surfaces on this path.
        let reply = call_with_timeout("GNOME List call failed", Some(INSTALL_HINT), move || {
            conn.call_method(Some(DEST), PATH, Some(IFACE), "List", &())
        })?;
        let json: String = reply
            .body()
            .deserialize()
            .context("GNOME List: unexpected D-Bus reply body from Window Calls extension")?;
        let windows: Vec<GnomeWindow> = serde_json::from_str(&json)
            .context("GNOME List: could not parse Window Calls JSON response")?;
        Ok(windows
            .into_iter()
            .filter(|w| matches_query(w, query))
            .map(window_info_from_gnome_window)
            .collect())
    }

    fn activate(&self, id: &str) -> anyhow::Result<()> {
        let winid = parse_window_id(id)?;
        let conn = self.conn.clone();
        call_with_timeout("GNOME Activate call failed", None, move || {
            conn.call_method(Some(DEST), PATH, Some(IFACE), "Activate", &(winid,))
        })?;
        Ok(())
    }
}

fn parse_window_id(id: &str) -> anyhow::Result<u32> {
    id.parse()
        .map_err(|_| anyhow::anyhow!("invalid GNOME window id: {id:?}"))
}

#[derive(serde::Deserialize)]
struct GnomeWindow {
    id: u32,
    // Mutter's get_title()/get_wm_class() are nullable, and the extension
    // passes their result through unchanged — so these can be JSON null.
    title: Option<String>,
    wm_class: Option<String>,
    // Tolerate the field being absent (extension version skew) so one odd
    // window doesn't abort the whole List parse.
    #[serde(default)]
    workspace: Option<i32>,
}

fn matches_query(w: &GnomeWindow, query: &str) -> bool {
    if query.is_empty() {
        return true;
    }
    let query = query.to_lowercase();
    let field_matches = |f: &Option<String>| {
        f.as_deref()
            .is_some_and(|s| s.to_lowercase().contains(&query))
    };
    field_matches(&w.title) || field_matches(&w.wm_class)
}

fn window_info_from_gnome_window(w: GnomeWindow) -> WindowInfo {
    // Mutter workspace indices are 0-based (-1 for sticky windows); show
    // them 1-based to match KWin's "Desktop 1" convention.
    let subtext = match w.workspace {
        Some(ws) if ws >= 0 => format!("Activate running window on workspace {}", ws + 1),
        _ => "Activate running window".to_string(),
    };
    WindowInfo {
        id: w.id.to_string(),
        title: w.title.unwrap_or_default(),
        icon: String::new(),
        subtext,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn window(id: u32, title: &str, wm_class: &str, workspace: i32) -> GnomeWindow {
        GnomeWindow {
            id,
            title: Some(title.to_string()),
            wm_class: Some(wm_class.to_string()),
            workspace: Some(workspace),
        }
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
    fn maps_fields_and_synthesizes_one_based_workspace_subtext() {
        let w = window(42, "My Window", "myapp", 3);
        let info = window_info_from_gnome_window(w);
        assert_eq!(info.id, "42");
        assert_eq!(info.title, "My Window");
        assert_eq!(info.icon, "");
        assert_eq!(info.subtext, "Activate running window on workspace 4");
    }

    #[test]
    fn sticky_window_workspace_gets_generic_subtext() {
        let info = window_info_from_gnome_window(window(1, "Sticky", "app", -1));
        assert_eq!(info.subtext, "Activate running window");
    }

    #[test]
    fn missing_workspace_gets_generic_subtext() {
        let json = r#"[{"id": 5, "title": "T", "wm_class": "c"}]"#;
        let windows: Vec<GnomeWindow> = serde_json::from_str(json).unwrap();
        let info = window_info_from_gnome_window(windows.into_iter().next().unwrap());
        assert_eq!(info.subtext, "Activate running window");
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
        assert_eq!(windows[0].title.as_deref(), Some("Term"));
    }

    #[test]
    fn missing_id_fails_to_deserialize() {
        let json = r#"[{"title": "T", "wm_class": "c", "workspace": 0}]"#;
        assert!(serde_json::from_str::<Vec<GnomeWindow>>(json).is_err());
    }

    #[test]
    fn null_title_and_wm_class_deserialize_and_do_not_match_queries() {
        let json = r#"[{"id": 9, "title": null, "wm_class": null, "workspace": 0}]"#;
        let windows: Vec<GnomeWindow> = serde_json::from_str(json).unwrap();
        assert_eq!(windows.len(), 1);
        let w = &windows[0];
        assert!(matches_query(w, ""));
        assert!(!matches_query(w, "firefox"));
    }

    #[test]
    fn null_title_still_matches_on_wm_class() {
        let w = GnomeWindow {
            id: 9,
            title: None,
            wm_class: Some("firefox".to_string()),
            workspace: Some(0),
        };
        assert!(matches_query(&w, "Firefox"));
    }

    #[test]
    fn null_title_maps_to_empty_string() {
        let w = GnomeWindow {
            id: 9,
            title: None,
            wm_class: None,
            workspace: Some(0),
        };
        let info = window_info_from_gnome_window(w);
        assert_eq!(info.title, "");
    }

    #[test]
    fn numeric_window_id_parses() {
        assert_eq!(parse_window_id("42").unwrap(), 42);
    }

    #[test]
    fn non_numeric_window_id_is_rejected_with_id_in_message() {
        let err = parse_window_id("id-1").unwrap_err();
        assert!(err.to_string().contains("id-1"));
    }

    #[test]
    fn negative_window_id_is_rejected() {
        assert!(parse_window_id("-3").is_err());
    }
}
