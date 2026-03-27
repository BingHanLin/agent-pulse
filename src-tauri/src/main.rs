// Prevents additional console window on Windows in release
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    if std::env::args().any(|a| a == "--cleanup") {
        agent_pulse::cleanup();
        return;
    }
    agent_pulse::run();
}
