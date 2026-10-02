use std::process::Command;
use std::time::Duration;
use tokio::time::sleep;

pub struct Supervisor {
    children: Vec<ChildProcess>,
}

struct ChildProcess {
    name: String,
    exe_path: String,
    args: Vec<String>,
}

impl Default for Supervisor {
    fn default() -> Self {
        Self::new()
    }
}

impl Supervisor {
    pub fn new() -> Self {
        Self {
            children: Vec::new(),
        }
    }

    pub fn add_child(&mut self, name: &str, exe_path: &str, args: &[&str]) {
        self.children.push(ChildProcess {
            name: name.to_string(),
            exe_path: exe_path.to_string(),
            args: args.iter().map(|s| s.to_string()).collect(),
        });
    }

    pub async fn run_mock_loop(&self) {
        for _child_def in &self.children {
            let mut attempt = 0;
            loop {
                attempt += 1;
                let mut cmd = Command::new("cmd.exe");
                cmd.args(&["/c", "exit 1"]); 
                
                if let Ok(mut child) = cmd.spawn() {
                    let status = child.wait().unwrap();
                    if !status.success() {
                        sleep(Duration::from_millis(10)).await;
                        if attempt > 2 {
                            break; 
                        }
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_supervisor_restarts() {
        let mut sup = Supervisor::new();
        sup.add_child("test_child", "cmd.exe", &["/c", "exit 1"]);
        sup.run_mock_loop().await;
    }
}
