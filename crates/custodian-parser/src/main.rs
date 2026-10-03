use std::env;
use std::fs;
use serde::Serialize;

#[derive(Serialize)]
struct ParseResult {
    file_type: String,
    content: String,
}

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: custodian-parser <file_path>");
        std::process::exit(1);
    }
    
    let path = &args[1];
    
    let extension = path.split('.').next_back().unwrap_or("unknown");
    let content = match fs::read_to_string(path) {
        Ok(c) => c,
        Err(_) => "Mock extracted text".to_string(),
    };
    
    let result = ParseResult {
        file_type: extension.to_string(),
        content,
    };
    
    println!("{}", serde_json::to_string(&result).unwrap());
}
