//! CLI output helpers: ASCII prefixes (OK / WARN / ERR / ->), ANSI colors
//! honoring NO_COLOR / --no-color (PRD 3.8-B), human vs --json duality.

use std::io::{IsTerminal, Write};

pub struct Out {
    pub json_mode: bool,
    pub color: bool,
}

impl Out {
    pub fn new(json_flag: bool, no_color: bool) -> Self {
        let color =
            !no_color && std::env::var_os("NO_COLOR").is_none() && std::io::stdout().is_terminal();
        Out {
            json_mode: json_flag || !std::io::stdout().is_terminal(),
            color,
        }
    }

    fn paint(&self, code: &str, text: &str) -> String {
        if self.color {
            format!("\x1b[{}m{}\x1b[0m", code, text)
        } else {
            text.to_string()
        }
    }

    pub fn ok(&self, s: &str) {
        println!("{} {}", self.paint("32", "OK"), s);
    }

    pub fn warn(&self, s: &str) {
        println!("{} {}", self.paint("33", "WARN"), s);
    }

    pub fn err(&self, s: &str) {
        let _ = std::io::stdout().flush();
        eprintln!("{} {}", self.paint("31", "ERR"), s);
    }

    pub fn info(&self, s: &str) {
        println!("{} {}", self.paint("36", "->"), s);
    }

    pub fn plain(&self, s: &str) {
        println!("{}", s);
    }

    pub fn print_json(&self, v: &impl serde::Serialize) {
        match serde_json::to_string_pretty(v) {
            Ok(s) => println!("{}", s),
            Err(e) => self.err(&format!("json serialize failed: {}", e)),
        }
    }
}

/// Read input from a path or stdin ("-").
pub fn read_input(path: &str) -> std::io::Result<(String, Option<std::path::PathBuf>)> {
    if path == "-" {
        use std::io::Read;
        let mut buf = String::new();
        std::io::stdin().read_to_string(&mut buf)?;
        Ok((buf, None))
    } else {
        let p = std::path::PathBuf::from(path);
        let content = std::fs::read_to_string(&p)?;
        Ok((content, p.parent().map(|d| d.to_path_buf())))
    }
}
