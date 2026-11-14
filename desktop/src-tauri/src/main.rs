#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
fn main() {
    if let Some(code) = creatorx_lib::cli::run_if_requested() {
        std::process::exit(code);
    }
    creatorx_lib::run();
}
