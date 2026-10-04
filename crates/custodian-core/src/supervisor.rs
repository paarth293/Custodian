use std::io;
use std::time::Duration;
use tokio::process::Command;
use tokio::task::JoinSet;
use tokio::time::sleep;

/// Restarts allowed for a crashing child after its first launch.
pub const MAX_RESTARTS: u32 = 3;
const RESTART_BACKOFF: Duration = Duration::from_millis(10);

#[derive(Debug, Clone)]
struct ChildSpec {
    exe_path: String,
    args: Vec<String>,
}

impl ChildSpec {
    /// Runs the child until it exits successfully or exhausts its restarts.
    /// Returns the number of launches.
    async fn supervise(&self) -> io::Result<u32> {
        let mut launches = 0;
        loop {
            launches += 1;
            let status = Command::new(&self.exe_path)
                .args(&self.args)
                .status()
                .await?;
            if status.success() || launches > MAX_RESTARTS {
                return Ok(launches);
            }
            sleep(RESTART_BACKOFF).await;
        }
    }
}

/// Runs child processes concurrently, restarting each on failure up to
/// [`MAX_RESTARTS`] times.
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

    /// Starts every child at once and waits for all of them to finish.
    /// Returns the number of launches per child, in registration order.
    pub async fn run(&self) -> io::Result<Vec<u32>> {
        let mut tasks = JoinSet::new();
        for (index, child) in self.children.iter().cloned().enumerate() {
            tasks.spawn(async move { (index, child.supervise().await) });
        }

        let mut launches = vec![0; self.children.len()];
        while let Some(joined) = tasks.join_next().await {
            let (index, result) = joined.map_err(io::Error::other)?;
            launches[index] = result?;
        }
        Ok(launches)
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

        assert_eq!(sup.run().await.unwrap(), vec![1 + MAX_RESTARTS, 1]);
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn runs_children_concurrently() {
        let mut sup = Supervisor::new();
        sup.add_child("sleep", &["0.5"]);
        sup.add_child("sleep", &["0.5"]);

        let started = std::time::Instant::now();
        sup.run().await.unwrap();
        assert!(started.elapsed() < Duration::from_millis(900));
    }
}
