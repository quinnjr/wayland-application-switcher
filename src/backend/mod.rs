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

pub fn detect_backend() -> anyhow::Result<Box<dyn WindowBackend>> {
    Ok(Box::new(kwin::KWinBackend::connect()?))
}

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
}
