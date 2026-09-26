// Prevents an additional console window on Windows in release.
#![cfg_attr(all(not(debug_assertions), windows), windows_subsystem = "windows")]

fn main() {
    pixivflow_desktop::run();
}
