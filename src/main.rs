mod app;
mod home;
mod icons;
mod keyboard;
mod mouse;
mod pedal;
mod shot;
mod theme;
mod widgets;

use std::path::PathBuf;

/// Startup flags. `--demo` uses the in-memory mouse and keyboard and the
/// simulated pedal; `--screen`, `--tab` and `--shot` drive the window
/// unattended for screenshots.
#[derive(Default, Clone)]
pub struct Flags {
    pub demo: bool,
    pub screen: Option<String>,
    pub tab: Option<String>,
    pub shot: Option<PathBuf>,
}

fn main() {
    let mut f = Flags::default();
    let mut it = std::env::args().skip(1);
    while let Some(a) = it.next() {
        match a.as_str() {
            "--demo" => f.demo = true,
            "--screen" => f.screen = it.next(),
            "--tab" => f.tab = it.next(),
            "--shot" => f.shot = it.next().map(PathBuf::from),
            "-h" | "--help" => {
                println!(
                    "octo-hub [--demo] [--screen home|mouse|keyboard|pedal] [--tab NAME] [--shot FILE.png]"
                );
                return;
            }
            other => {
                eprintln!("unknown argument {other}; try --help");
                std::process::exit(2);
            }
        }
    }
    if let Err(e) = app::run(f) {
        eprintln!("error: {e:#}");
        std::process::exit(1);
    }
}
