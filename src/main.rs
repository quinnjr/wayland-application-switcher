mod backend;
mod picker;
mod resolve;

fn main() {
    use picker::Picker as _;
    let picker = picker::TuiPicker;
    let windows = vec![
        backend::WindowInfo {
            id: "1".into(),
            title: "Firefox".into(),
            icon: String::new(),
            subtext: "Desktop 1".into(),
        },
        backend::WindowInfo {
            id: "2".into(),
            title: "Konsole".into(),
            icon: String::new(),
            subtext: "Desktop 1".into(),
        },
    ];
    println!("picked: {:?}", picker.pick(&windows));
}
