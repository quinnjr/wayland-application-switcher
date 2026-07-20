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
        Ok(Self {
            conn: Connection::session()?,
        })
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

#[derive(serde::Deserialize)]
struct GnomeWindow {
    id: u32,
    title: String,
    wm_class: String,
    workspace: i32,
}

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
