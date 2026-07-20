use crate::backend::WindowBackend;
use crate::picker::Picker;

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::WindowInfo;
    use std::cell::RefCell;

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
