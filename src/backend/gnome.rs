use crate::backend::WindowInfo;

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
