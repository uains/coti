//! Coti executable entry point.

fn main() -> anyhow::Result<()> {
    let mut gui = false;
    let mut path = None;
    for argument in std::env::args().skip(1) {
        if argument == "--gui" {
            gui = true;
        } else if !argument.starts_with('-') {
            path = Some(std::path::PathBuf::from(argument));
        }
    }
    if gui || path.is_some() {
        md_app::run_gui_with_path(path)
    } else {
        md_app::run()
    }
}
