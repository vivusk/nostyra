#![deny(unsafe_code)]
// Release builds are GUI apps: no console window on Windows.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod startup_error_dialog;

fn main() -> anyhow::Result<()> {
    nostyra_launcher_lib::run().inspect_err(startup_error_dialog::show)
}
