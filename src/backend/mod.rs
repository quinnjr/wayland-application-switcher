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

pub mod gnome;
pub mod kwin;

pub enum MatchResolution {
    None,
    One(WindowInfo),
    Ambiguous(Vec<WindowInfo>),
}

pub fn classify_matches(mut matches: Vec<WindowInfo>) -> MatchResolution {
    match matches.len() {
        0 => MatchResolution::None,
        1 => MatchResolution::One(matches.remove(0)),
        _ => MatchResolution::Ambiguous(matches),
    }
}

#[derive(Debug)]
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

#[cfg(test)]
mod tests {
    use super::*;

    fn window(id: &str) -> WindowInfo {
        WindowInfo {
            id: id.to_string(),
            title: format!("Window {id}"),
            icon: String::new(),
            subtext: String::new(),
        }
    }

    #[test]
    fn zero_matches_classifies_as_none() {
        assert!(matches!(classify_matches(vec![]), MatchResolution::None));
    }

    #[test]
    fn single_match_classifies_as_one() {
        match classify_matches(vec![window("1")]) {
            MatchResolution::One(w) => assert_eq!(w.id, "1"),
            _ => panic!("expected MatchResolution::One"),
        }
    }

    #[test]
    fn multiple_matches_classify_as_ambiguous() {
        match classify_matches(vec![window("1"), window("2")]) {
            MatchResolution::Ambiguous(matches) => {
                assert_eq!(matches[0].id, "1");
                assert_eq!(matches[1].id, "2");
            }
            _ => panic!("expected MatchResolution::Ambiguous"),
        }
    }

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
        assert!(matches!(
            backend_for_desktop("ubuntu:GNOME"),
            Ok(Backend::Gnome)
        ));
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
}
