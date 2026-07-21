use crate::backend::WindowInfo;
use ntui::{
    BorderStyle, Color, Element, FlexDirection, KeyCode, Weight, component, element, render,
};
use std::rc::Rc;
use std::sync::{Arc, Mutex};

pub trait Picker {
    fn pick(&self, windows: &[WindowInfo]) -> Option<String>;
}

pub struct TuiPicker;

#[derive(Clone, Default)]
struct ResultSlot(Arc<Mutex<Option<String>>>);

impl PartialEq for ResultSlot {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}

#[derive(Clone, PartialEq, Default)]
struct PickerViewProps {
    windows: Rc<[WindowInfo]>,
    result: ResultSlot,
}

#[component]
fn PickerView(props: &PickerViewProps, hooks: &mut ntui::Hooks) -> ntui::Element {
    let selected = hooks.use_state(|| 0usize);
    let app = hooks.use_app();

    let windows = props.windows.clone();
    let result = props.result.clone();
    let sel = selected.clone();
    hooks.use_input(move |ev, _| {
        let len = windows.len();
        match ev.code {
            KeyCode::Down => {
                let i = sel.get();
                sel.set((i + 1).min(len.saturating_sub(1)));
            }
            KeyCode::Up => {
                let i = sel.get();
                sel.set(i.saturating_sub(1));
            }
            KeyCode::Enter => {
                let i = sel.get();
                *result.0.lock().unwrap_or_else(|e| e.into_inner()) =
                    windows.get(i).map(|w| w.id.clone());
                app.exit();
            }
            KeyCode::Esc => {
                *result.0.lock().unwrap_or_else(|e| e.into_inner()) = None;
                app.exit();
            }
            _ => {}
        }
    });

    let idx = selected.get();
    element! {
        View(flex_direction: FlexDirection::Column, padding: 1, border_style: BorderStyle::Single) {
            Text(content: "was — pick a window (up/down + enter, esc to cancel)", color: Color::DarkGrey)
            #(props.windows.iter().enumerate().map(|(i, w)| {
                let marker = if i == idx { ">" } else { " " };
                element! {
                    Text(content: format!("{marker} {}  {}", w.title, w.subtext),
                         weight: if i == idx { Weight::Bold } else { Weight::Normal },
                         key: w.id.clone())
                }
            }))
        }
    }
}

impl Picker for TuiPicker {
    fn pick(&self, windows: &[WindowInfo]) -> Option<String> {
        let result = ResultSlot::default();
        let props = PickerViewProps {
            windows: Rc::from(windows),
            result: result.clone(),
        };
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("failed to start ntui runtime");
        rt.block_on(render(Element::component::<PickerView>(props)))
            .expect("ntui render failed");
        result.0.lock().unwrap_or_else(|e| e.into_inner()).clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ntui::KeyCode;
    use ntui::testing::TestTerminal;

    fn window(id: &str, title: &str) -> WindowInfo {
        WindowInfo {
            id: id.to_string(),
            title: title.to_string(),
            icon: String::new(),
            subtext: "Desktop 1".to_string(),
        }
    }

    fn harness(windows: Vec<WindowInfo>) -> (TestTerminal, ResultSlot) {
        let result = ResultSlot::default();
        let props = PickerViewProps {
            windows: Rc::from(windows),
            result: result.clone(),
        };
        let terminal = TestTerminal::new(60, 10, Element::component::<PickerView>(props)).unwrap();
        (terminal, result)
    }

    #[tokio::test]
    async fn initial_frame_selects_first_window() {
        let (t, _result) = harness(vec![window("1", "Firefox"), window("2", "Konsole")]);
        let frame = t.frame_text();
        assert!(frame.contains("> Firefox"));
        assert!(!frame.contains("> Konsole"));
    }

    #[tokio::test]
    async fn down_moves_selection_to_next_window() {
        let (mut t, _result) = harness(vec![window("1", "Firefox"), window("2", "Konsole")]);
        t.send_key(KeyCode::Down).unwrap();
        let frame = t.frame_text();
        assert!(frame.contains("> Konsole"));
        assert!(!frame.contains("> Firefox"));
    }

    #[tokio::test]
    async fn down_does_not_move_past_last_window() {
        let (mut t, _result) = harness(vec![window("1", "Firefox"), window("2", "Konsole")]);
        t.send_key(KeyCode::Down).unwrap();
        t.send_key(KeyCode::Down).unwrap();
        let frame = t.frame_text();
        assert!(frame.contains("> Konsole"));
    }

    #[tokio::test]
    async fn enter_selects_current_window_and_exits() {
        let (mut t, result) = harness(vec![window("1", "Firefox"), window("2", "Konsole")]);
        t.send_key(KeyCode::Down).unwrap();
        t.send_key(KeyCode::Enter).unwrap();
        assert!(t.exited());
        assert_eq!(result.0.lock().unwrap().as_deref(), Some("2"));
    }

    #[tokio::test]
    async fn esc_cancels_and_exits_with_no_result() {
        let (mut t, result) = harness(vec![window("1", "Firefox"), window("2", "Konsole")]);
        t.send_key(KeyCode::Esc).unwrap();
        assert!(t.exited());
        assert_eq!(*result.0.lock().unwrap(), None);
    }
}
