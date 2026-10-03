use custodian_agent::tools::ToolRegistry;
use std::process::Command;

fn main() {
    println!("Spike I: Tool dynamic execution.");
    
    let json = r#"[{"name": "git", "description": "Git VCS", "command": "git"}]"#;
    let mut registry = ToolRegistry::new();
    registry.load_from_json(json).expect("Failed to load tools");
    
    if let Some(tool) = registry.get_tool("git") {
        let output = Command::new(&tool.command)
            .arg("--version")
            .output()
            .expect("Failed to execute tool");
            
        println!("Executed {}: {}", tool.name, String::from_utf8_lossy(&output.stdout));
    }
    
    println!("Spike I completed.");
}
