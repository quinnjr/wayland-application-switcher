use crate::backend::WindowBackend;
use crate::backend::detect_backend;

pub fn run_list(query: Option<&str>) -> anyhow::Result<()> {
    let backend = detect_backend()?;
    let windows = list_windows(backend.as_ref(), query)?;
    for w in &windows {
        println!("{}", format_row(w));
    }
    Ok(())
}

// README documents the output contract as `id<TAB>title<TAB>subtext`.
fn format_row(w: &crate::backend::WindowInfo) -> String {
    format!("{}\t{}\t{}", w.id, w.title, w.subtext)
}

fn list_windows(
    backend: &dyn WindowBackend,
    query: Option<&str>,
) -> anyhow::Result<Vec<crate::backend::WindowInfo>> {
    backend.list_windows(query.unwrap_or(""))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::WindowInfo;

    struct FakeBackend {
        expected_query: &'static str,
        windows: Vec<WindowInfo>,
    }

    impl WindowBackend for FakeBackend {
        fn list_windows(&self, query: &str) -> anyhow::Result<Vec<WindowInfo>> {
            assert_eq!(query, self.expected_query);
            Ok(self.windows.clone())
        }
        fn activate(&self, _id: &str) -> anyhow::Result<()> {
            panic!("list should never activate a window");
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

    #[test]
    fn no_query_passes_through_as_empty_string_for_all_windows() {
        let backend = FakeBackend {
            expected_query: "",
            windows: vec![window("1"), window("2")],
        };
        let windows = list_windows(&backend, None).unwrap();
        assert_eq!(windows.len(), 2);
    }

    #[test]
    fn some_query_passes_through_verbatim() {
        let backend = FakeBackend {
            expected_query: "konsole",
            windows: vec![window("1")],
        };
        let windows = list_windows(&backend, Some("konsole")).unwrap();
        assert_eq!(windows.len(), 1);
    }

    #[test]
    fn rows_are_tab_separated_id_title_subtext() {
        let w = WindowInfo {
            id: "42".to_string(),
            title: "My Window".to_string(),
            icon: "unused".to_string(),
            subtext: "Activate running window on Desktop 1".to_string(),
        };
        assert_eq!(
            format_row(&w),
            "42\tMy Window\tActivate running window on Desktop 1"
        );
    }
}
