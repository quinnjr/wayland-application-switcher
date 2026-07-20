mod backend;
mod picker;
mod resolve;

fn main() {
    let backend = backend::detect_backend().expect("backend detect failed");
    let windows = backend.list_windows("konsole").expect("list_windows failed");
    for w in &windows {
        println!("{:?}", w);
    }
}
