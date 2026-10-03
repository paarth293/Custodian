use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ToolDefinition {
    pub name: String,
    pub description: String,
    pub command: String,
}

pub struct ToolRegistry {
    tools: HashMap<String, ToolDefinition>,
}

impl Default for ToolRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl ToolRegistry {
    pub fn new() -> Self {
        Self {
            tools: HashMap::new(),
        }
    }

    pub fn load_from_json(&mut self, json_data: &str) -> Result<(), serde_json::Error> {
        let parsed: Vec<ToolDefinition> = serde_json::from_str(json_data)?;
        for tool in parsed {
            self.tools.insert(tool.name.clone(), tool);
        }
        Ok(())
    }

    pub fn get_tool(&self, name: &str) -> Option<&ToolDefinition> {
        self.tools.get(name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_load_tools() {
        let json = r#"[{"name": "search", "description": "Web search", "command": "curl"}]"#;
        let mut registry = ToolRegistry::new();
        registry.load_from_json(json).unwrap();

        let tool = registry.get_tool("search").unwrap();
        assert_eq!(tool.description, "Web search");
    }
}
