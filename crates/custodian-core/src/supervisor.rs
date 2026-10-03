use std::io;
use std::time::Duration;
use tokio::process::Command;
use tokio::time::sleep;

/// Maximum restarts attempted for a crashing child before giving up.
pub const MAX_RESTARTS: u32 = 3;
const RESTART_BACKOFF: Duration = Duration::from_millis(10);

#[derive(Debug, Clone)]
struct ChildSpec {
    exe_path: String,
    args: Vec<String>,
}

/// Starts child processes and restarts them on failure, up to [`MAX_RESTARTS`].
#[derive(Debug, Default)]
pub struct Supervisor {
    children: Vec<ChildSpec>,
}

impl Supervisor {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_child(&mut self, exe_path: &str, args: &[&str]) {
        self.children.push(ChildSpec {
            exe_path: exe_path.to_string(),
            args: args.iter().map(|s| s.to_string()).collect(),
        });
    }

    /// Runs every child to completion and returns how many attempts each took.
    pub async fn run(&self) -> io::Result<Vec<u32>> {
        let mut attempts = Vec::with_capacity(self.children.len());
        for child in &self.children {
            let mut attempt = 0;
            loop {
                attempt += 1;
                let status = Command::new(&child.exe_path)
                    .args(&child.args)
                    .status()
                    .await?;
                if status.success() || attempt >= MAX_RESTARTS {
                    break;
                }
                sleep(RESTART_BACKOFF).await;
            }
            attempts.push(attempt);
        }
        Ok(attempts)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(windows)]
    fn failing() -> (&'static str, &'static [&'static str]) {
        ("cmd.exe", &["/c", "exit 1"])
    }
    #[cfg(not(windows))]
    fn failing() -> (&'static str, &'static [&'static str]) {
        ("false", &[])
    }

    #[cfg(windows)]
    fn succeeding() -> (&'static str, &'static [&'static str]) {
        ("cmd.exe", &["/c", "exit 0"])
    }
    #[cfg(not(windows))]
    fn succeeding() -> (&'static str, &'static [&'static str]) {
        ("true", &[])
    }

    #[tokio::test]
    async fn restarts_failing_child_up_to_limit() {
        let mut sup = Supervisor::new();
        let (exe, args) = failing();
        sup.add_child(exe, args);
        let (exe, args) = succeeding();
        sup.add_child(exe, args);

        assert_eq!(sup.run().await.unwrap(), vec![MAX_RESTARTS, 1]);
    }
}
