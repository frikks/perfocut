// Без чёрного окна консоли в Windows
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    if let Err(e) = perfocut::run() {
        eprintln!("PerfoCut: {e}");
        std::process::exit(1);
    }
}
