#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    // Dual-purpose binary: the GUI exe also answers `mcp serve` so a
    // standalone install still works when no sibling CLI exists.
    let args: Vec<String> = std::env::args().collect();
    if args.len() >= 3 && args[1] == "mcp" && args[2] == "serve" {
        if let Err(e) = wxwright_mcp::run_stdio() {
            eprintln!("mcp serve error: {}", e);
            std::process::exit(2);
        }
        return;
    }
    wxwright_gui::run()
}
