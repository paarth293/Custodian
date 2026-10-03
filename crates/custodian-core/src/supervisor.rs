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

    /// Mock restart loop: runs every child up to three times, backing off between
    /// failures, and returns the number of attempts made per child name.
    pub async fn run_mock_loop(&self) -> Vec<(String, u32)> {
        let mut attempts = Vec::new();
        for child_def in &self.children {
            let mut attempt = 0;
            loop {
                attempt += 1;
                if let Ok(mut child) = Command::new(&child_def.exe_path)
                    .args(&child_def.args)
                    .spawn()
                {
                    if child.wait().is_ok_and(|status| status.success()) {
                        break;
                    }
                }
                sleep(Duration::from_millis(10)).await;
                if attempt > 2 {
                    break;
                }
            }
            attempts.push((child_def.name.clone(), attempt));
        }
        attempts
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_supervisor_restarts() {
        let mut sup = Supervisor::new();
        sup.add_child("test_child", "cmd.exe", &["/c", "exit 1"]);
        let attempts = sup.run_mock_loop().await;
        assert_eq!(attempts, vec![("test_child".to_string(), 3)]);
    }
}
