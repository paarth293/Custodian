//! Sandboxed parser: reads one file and prints `{"file_type", "content"}` JSON.

use serde::Serialize;
use std::path::Path;
use std::process::ExitCode;

#[derive(Serialize)]
struct ParseResult {
    file_type: String,
    content: String,
}

fn main() -> ExitCode {
    let Some(path) = std::env::args().nth(1) else {
        eprintln!("usage: custodian-parser <file_path>");
        return ExitCode::from(1);
    };
    let path = Path::new(&path);

    let content = match std::fs::read(path) {
        Ok(bytes) => String::from_utf8_lossy(&bytes).into_owned(),
        Err(err) => {
            eprintln!("cannot read {}: {err}", path.display());
            return ExitCode::from(2);
        }
    };

    let result = ParseResult {
        file_type: path
            .extension()
            .map(|e| e.to_string_lossy().to_lowercase())
            .unwrap_or_else(|| "unknown".into()),
        content,
    };

    match serde_json::to_string(&result) {
        Ok(json) => {
            println!("{json}");
            ExitCode::SUCCESS
        }
        Err(err) => {
            eprintln!("serialization failed: {err}");
            ExitCode::from(3)
        }
    }
}
