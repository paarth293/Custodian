//! Client for the agent process (`services/agent`), which runs the local
//! language model. Messages are HMAC-authenticated frames (`custodian-ipc`)
//! over the child's stdin/stdout, with a fresh random key per launch.
//!
//! The model can ask for Cubby's tools: the agent sends a `tool` frame, the
//! desktop runs the tool (it, not the model, decides what that means) and
//! answers with a `tool_result` frame.
//!
//! On Windows the agent runs inside an AppContainer (§88 layer 1, SR-B2): it
//! cannot reach the network or read the person's files, and can read only
//! Node, its own code and packages, and the model files.

use custodian_ipc::{read_frame, write_frame, IpcError};
use custodian_winsec::job::{Job, JobLimits};
use rand::RngCore;
use serde::{Deserialize, Serialize};
use std::io::{BufReader, BufWriter, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum AgentError {
    #[error("the agent could not be started: {0}")]
    Spawn(String),
    #[error("lost contact with the agent: {0}")]
    Ipc(#[from] IpcError),
    #[error("{0}")]
    Model(String),
    #[error("agent operation timed out: {0}")]
    Timeout(String),
}

#[derive(Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum Request<'a> {
    Chat {
        id: u64,
        message: &'a str,
    },
    ToolResult {
        id: u64,
        call: u64,
        result: &'a str,
    },
    /// The person pressed Stop: end this answer now.
    Cancel {
        id: u64,
    },
    /// "Can you work here?", answered at once.
    Ping {
        id: u64,
    },
    /// Reset conversation context.
    #[allow(dead_code)]
    Reset {
        id: u64,
    },
}

/// What a tool call gets once the person has pressed Stop.
const STOPPED: &str = "The person stopped this request. Do not continue.";

/// Maximum time to wait for the agent to acknowledge Stop / Cancel before considering it unresponsive.
const CANCEL_DEADLINE: std::time::Duration = std::time::Duration::from_secs(3);

/// Maximum time to wait for a ping response before timing out.
const PING_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(30);

/// Maximum time of total silence during chat before watchdog trips.
const CHAT_IDLE_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(120);

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum Reply {
    Token {
        id: u64,
        text: String,
    },
    Done {
        id: u64,
    },
    Error {
        id: u64,
        error: String,
    },
    Tool {
        id: u64,
        call: u64,
        name: String,
        #[serde(default)]
        args: serde_json::Value,
    },
    Pong {
        id: u64,
        ready: bool,
        #[serde(default)]
        error: Option<String>,
    },
}

/// Runs a tool the model asked for and returns the result for the model.
pub type ToolHandler<'a> = &'a mut dyn FnMut(&str, &serde_json::Value) -> String;

/// One request/response conversation channel to an agent.
pub struct Connection<R: Read + Send + 'static, W: Write> {
    receiver: std::sync::mpsc::Receiver<Result<Reply, AgentError>>,
    writer: BufWriter<W>,
    key: Vec<u8>,
    next_id: u64,
    _marker: std::marker::PhantomData<R>,
}

impl<R: Read + Send + 'static, W: Write> Connection<R, W> {
    pub fn new(reader: R, writer: W, key: Vec<u8>) -> Self {
        let (tx, rx) = std::sync::mpsc::channel();
        let key_clone = key.clone();
        std::thread::Builder::new()
            .name("custodian-agent-ipc-reader".into())
            .spawn(move || {
                let mut reader = BufReader::new(reader);
                loop {
                    match read_frame::<Reply, _>(&mut reader, &key_clone) {
                        Ok(reply) => {
                            if tx.send(Ok(reply)).is_err() {
                                break;
                            }
                        }
                        Err(IpcError::Io(e))
                            if e.kind() == std::io::ErrorKind::UnexpectedEof
                                || e.kind() == std::io::ErrorKind::BrokenPipe =>
                        {
                            break;
                        }
                        Err(e) => {
                            let _ = tx.send(Err(AgentError::Ipc(e)));
                            break;
                        }
                    }
                }
            })
            .expect("spawn agent ipc reader thread");

        Self {
            receiver: rx,
            writer: BufWriter::new(writer),
            key,
            next_id: 1,
            _marker: std::marker::PhantomData,
        }
    }

    /// Checks transport, file access and runtime import. Model inference readiness
    /// is verified by a deep probe or an actual chat, not this shallow ping.
    #[cfg(test)]
    pub fn ping(&mut self) -> Result<Option<String>, AgentError> {
        self.ping_with_cancel(None)
    }

    pub fn ping_with_cancel(
        &mut self,
        cancel: Option<&AtomicBool>,
    ) -> Result<Option<String>, AgentError> {
        let id = self.next_id;
        self.next_id += 1;
        write_frame(&mut self.writer, &Request::Ping { id }, &self.key)?;
        let start = std::time::Instant::now();
        loop {
            if cancel.is_some_and(|flag| flag.load(Ordering::Relaxed)) {
                return Err(AgentError::Timeout("agent startup was cancelled".into()));
            }
            if start.elapsed() > PING_TIMEOUT {
                return Err(AgentError::Timeout("agent ping timed out".into()));
            }
            match self
                .receiver
                .recv_timeout(std::time::Duration::from_millis(50))
            {
                Ok(Ok(Reply::Pong {
                    id: rid,
                    ready,
                    error,
                })) => {
                    if rid == id {
                        return Ok((!ready).then(|| error.unwrap_or_else(|| "not ready".into())));
                    }
                }
                Ok(Ok(_)) => {}
                Ok(Err(e)) => return Err(e),
                Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {}
                Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                    return Err(AgentError::Ipc(IpcError::Io(std::io::Error::new(
                        std::io::ErrorKind::UnexpectedEof,
                        "agent process disconnected",
                    ))));
                }
            }
        }
    }

    /// Resets the agent's chat history for a fresh conversation.
    #[allow(dead_code)]
    pub fn reset(&mut self) -> Result<(), AgentError> {
        let id = self.next_id;
        self.next_id += 1;
        write_frame(&mut self.writer, &Request::Reset { id }, &self.key)?;
        let start = std::time::Instant::now();
        loop {
            if start.elapsed() > std::time::Duration::from_secs(5) {
                return Err(AgentError::Timeout("agent reset timed out".into()));
            }
            match self
                .receiver
                .recv_timeout(std::time::Duration::from_millis(50))
            {
                Ok(Ok(Reply::Done { id: rid })) if rid == id => return Ok(()),
                Ok(Ok(Reply::Error { id: rid, error })) if rid == id => {
                    return Err(AgentError::Model(error))
                }
                Ok(Ok(_)) => {}
                Ok(Err(e)) => return Err(e),
                Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {}
                Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                    return Err(AgentError::Ipc(IpcError::Io(std::io::Error::new(
                        std::io::ErrorKind::UnexpectedEof,
                        "agent process disconnected",
                    ))));
                }
            }
        }
    }

    /// Sends a message, streams the reply's text chunks to `on_text`, and
    /// runs the tools the model asks for with `on_tool`. When `cancel` is set,
    /// the agent is told to stop; nothing more is shown and no more tools run.
    pub fn chat(
        &mut self,
        message: &str,
        cancel: &AtomicBool,
        on_text: &mut dyn FnMut(&str),
        on_tool: ToolHandler<'_>,
    ) -> Result<(), AgentError> {
        let id = self.next_id;
        self.next_id += 1;
        write_frame(&mut self.writer, &Request::Chat { id, message }, &self.key)?;
        let mut stopped = false;
        let mut cancel_sent_at: Option<std::time::Instant> = None;
        let mut last_activity = std::time::Instant::now();
        loop {
            if !stopped && cancel.load(Ordering::Relaxed) {
                let _ = write_frame(&mut self.writer, &Request::Cancel { id }, &self.key);
                stopped = true;
                cancel_sent_at = Some(std::time::Instant::now());
            }

            if let Some(sent_at) = cancel_sent_at {
                if sent_at.elapsed() > CANCEL_DEADLINE {
                    return Err(AgentError::Timeout(
                        "agent did not respond to cancel within deadline".into(),
                    ));
                }
            } else if last_activity.elapsed() > CHAT_IDLE_TIMEOUT {
                return Err(AgentError::Timeout("agent inactive for too long".into()));
            }

            match self
                .receiver
                .recv_timeout(std::time::Duration::from_millis(50))
            {
                Ok(Ok(reply)) => {
                    last_activity = std::time::Instant::now();
                    match reply {
                        Reply::Token { id: rid, text } if rid == id => {
                            if !stopped {
                                on_text(&text);
                            }
                        }
                        Reply::Done { id: rid } if rid == id => return Ok(()),
                        Reply::Error { id: rid, error } if rid == id => {
                            return Err(AgentError::Model(error))
                        }
                        Reply::Tool {
                            id: rid,
                            call,
                            name,
                            args,
                        } if rid == id => {
                            let result = if stopped {
                                STOPPED.to_string()
                            } else {
                                on_tool(&name, &args)
                            };
                            let reply = Request::ToolResult {
                                id,
                                call,
                                result: &result,
                            };
                            write_frame(&mut self.writer, &reply, &self.key)?;
                        }
                        // Replies to an earlier, abandoned request are ignored.
                        _ => {}
                    }
                }
                Ok(Err(err)) => return Err(err),
                Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                    // Check cancel / timeout conditions on next iteration
                }
                Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                    return Err(AgentError::Ipc(IpcError::Io(std::io::Error::new(
                        std::io::ErrorKind::UnexpectedEof,
                        "agent process disconnected",
                    ))));
                }
            }
        }
    }
}

/// Limits for the agent process: room for a small chat model and its
/// context, below-normal priority so the desktop stays responsive, and no CPU
/// cap, which would only slow replies down.
const AGENT_LIMITS: JobLimits = JobLimits {
    memory_mb: Some(4096),
    cpu_percent: None,
    below_normal_priority: true,
};

/// The agent's process, started normally or inside its sandbox.
enum Runner {
    Plain(Child),
    #[cfg(windows)]
    Contained(custodian_winsec::appcontainer::Contained),
}

/// A running agent process.
pub struct AgentProcess {
    runner: Runner,
    connection: Connection<Box<dyn Read + Send>, Box<dyn Write + Send>>,
    /// Holds the agent's limits and ends it if Custodian exits or crashes.
    _job: Option<Job>,
}

/// A fresh random IPC key, as the hex text the agent reads.
fn new_key() -> String {
    let mut key = vec![0u8; 32];
    rand::rngs::OsRng.fill_bytes(&mut key);
    key.iter().map(|b| format!("{b:02x}")).collect()
}

/// Settings the agent may be given, passed through when set.
const PASSED_THROUGH: &[&str] = &["CUSTODIAN_CHAT_MODEL", "CUSTODIAN_LLAMA_GPU"];

/// A job with the agent's limits. Returns an error if Windows refused so we fail closed.
fn agent_job(assign: impl FnOnce(&Job) -> std::io::Result<()>) -> Result<Job, AgentError> {
    Job::new(AGENT_LIMITS)
        .and_then(|job| assign(&job).map(|()| job))
        .map_err(|err| AgentError::Spawn(format!("could not configure job limits: {err}")))
}

impl AgentProcess {
    /// Starts `node <entry>` with a fresh IPC key, as a normal process.
    pub fn spawn(node: &str, entry: &Path, models_dir: Option<&Path>) -> Result<Self, AgentError> {
        let key_hex = new_key();
        let mut command = Command::new(node);
        command
            .env_clear()
            .arg(entry)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit());
        for var in ["SystemRoot", "windir", "PATH", "TEMP", "TMP"] {
            if let Some(val) = std::env::var_os(var) {
                command.env(var, val);
            }
        }
        command.env("CUSTODIAN_IPC_KEY", &key_hex);
        if let Some(dir) = models_dir {
            command.env("CUSTODIAN_MODELS_DIR", dir);
        }
        if std::env::var_os("CUSTODIAN_LLAMA_GPU").is_none() {
            command.env("CUSTODIAN_LLAMA_GPU", "vulkan");
        }
        command.env("DISABLE_LAYER_AMD_SWITCHABLE_GRAPHICS_1", "1");
        for name in PASSED_THROUGH {
            if let Some(value) = std::env::var_os(name) {
                command.env(name, value);
            }
        }
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            const CREATE_NO_WINDOW: u32 = 0x0800_0000;
            command.creation_flags(CREATE_NO_WINDOW);
        }

        let mut child = command
            .spawn()
            .map_err(|e| AgentError::Spawn(e.to_string()))?;
        let job = match agent_job(|job| job.assign(&child)) {
            Ok(job) => job,
            Err(err) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(err);
            }
        };
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| AgentError::Spawn("no stdout".into()))?;
        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| AgentError::Spawn("no stdin".into()))?;
        // The key is hex text on the agent side, so authenticate with its bytes.
        let connection: Connection<Box<dyn Read + Send>, Box<dyn Write + Send>> =
            Connection::new(Box::new(stdout), Box::new(stdin), key_hex.into_bytes());
        Ok(Self {
            runner: Runner::Plain(child),
            connection,
            _job: Some(job),
        })
    }

    /// Starts `node <entry>` inside `sandbox`, under its job before it runs.
    #[cfg(windows)]
    pub fn spawn_sandboxed(
        sandbox: &custodian_winsec::appcontainer::Sandbox,
        node: &Path,
        entry: &Path,
        models_dir: Option<&Path>,
    ) -> Result<Self, AgentError> {
        use std::ffi::{OsStr, OsString};
        let key_hex = new_key();
        let mut env: Vec<(&str, OsString)> = vec![("CUSTODIAN_IPC_KEY", OsString::from(&key_hex))];
        if let Some(dir) = models_dir {
            env.push(("CUSTODIAN_MODELS_DIR", dir.as_os_str().to_owned()));
        }
        if std::env::var_os("CUSTODIAN_LLAMA_GPU").is_none() {
            env.push(("CUSTODIAN_LLAMA_GPU", OsString::from("off")));
        }
        env.push((
            "DISABLE_LAYER_AMD_SWITCHABLE_GRAPHICS_1",
            OsString::from("1"),
        ));
        for name in PASSED_THROUGH {
            if let Some(value) = std::env::var_os(name) {
                env.push((name, value));
            }
        }
        let env: Vec<(&str, &OsStr)> = env.iter().map(|(k, v)| (*k, v.as_os_str())).collect();
        let private = sandbox.private_dir();
        // Resolving real paths makes Node look at every folder above the
        // script, which the sandbox may not see; the paths given are final.
        let args = [
            OsStr::new("--preserve-symlinks"),
            OsStr::new("--preserve-symlinks-main"),
            entry.as_os_str(),
        ];
        let mut contained = sandbox
            .spawn(node, &args, &env, private.as_deref())
            .map_err(|e| AgentError::Spawn(e.to_string()))?;
        let job = match agent_job(|job| job.assign_raw(contained.raw_process())) {
            Ok(job) => job,
            Err(err) => {
                contained.kill();
                return Err(err);
            }
        };
        if let Err(err) = contained.resume() {
            contained.kill();
            return Err(AgentError::Spawn(err.to_string()));
        }
        // The agent's log lines go to Custodian's own log.
        if let Some(mut log) = contained.stderr.take() {
            std::thread::spawn(move || {
                let _ = std::io::copy(&mut log, &mut std::io::stderr());
            });
        }
        let (Some(stdout), Some(stdin)) = (contained.stdout.take(), contained.stdin.take()) else {
            contained.kill();
            return Err(AgentError::Spawn("no pipes".into()));
        };
        let connection: Connection<Box<dyn Read + Send>, Box<dyn Write + Send>> =
            Connection::new(Box::new(stdout), Box::new(stdin), key_hex.into_bytes());
        Ok(Self {
            runner: Runner::Contained(contained),
            connection,
            _job: Some(job),
        })
    }

    /// Whether the agent runs inside its sandbox.
    pub fn sandboxed(&self) -> bool {
        match self.runner {
            Runner::Plain(_) => false,
            #[cfg(windows)]
            Runner::Contained(_) => true,
        }
    }

    #[cfg(test)]
    pub fn ping(&mut self) -> Result<Option<String>, AgentError> {
        self.connection.ping()
    }

    pub fn ping_with_cancel(
        &mut self,
        cancel: Option<&AtomicBool>,
    ) -> Result<Option<String>, AgentError> {
        self.connection.ping_with_cancel(cancel)
    }

    #[allow(dead_code)]
    pub fn reset(&mut self) -> Result<(), AgentError> {
        self.connection.reset()
    }

    #[allow(dead_code)]
    pub fn kill(&mut self) {
        match &mut self.runner {
            Runner::Plain(child) => {
                let _ = child.kill();
                let _ = child.wait();
            }
            #[cfg(windows)]
            Runner::Contained(child) => child.kill(),
        }
    }

    pub fn chat(
        &mut self,
        message: &str,
        cancel: &AtomicBool,
        on_text: &mut dyn FnMut(&str),
        on_tool: ToolHandler<'_>,
    ) -> Result<(), AgentError> {
        self.connection.chat(message, cancel, on_text, on_tool)
    }

    pub fn is_alive(&mut self) -> bool {
        match &mut self.runner {
            Runner::Plain(child) => matches!(child.try_wait(), Ok(None)),
            #[cfg(windows)]
            Runner::Contained(child) => matches!(child.try_wait(), Ok(None)),
        }
    }
}

impl Drop for AgentProcess {
    fn drop(&mut self) {
        match &mut self.runner {
            Runner::Plain(child) => {
                let _ = child.kill();
                let _ = child.wait();
            }
            #[cfg(windows)]
            Runner::Contained(child) => child.kill(),
        }
    }
}

/// The folders the agent's sandbox may read: Node's own folder, the agent
/// package (its code and package.json), the `node_modules` holding its model
/// runtime, and the models. Nothing of the person's.
pub fn sandbox_read_roots(node: &Path, entry: &Path, models_dir: Option<&Path>) -> Vec<PathBuf> {
    let mut roots = Vec::new();
    if let Some(dir) = node.parent() {
        roots.push(dir.to_path_buf());
    }
    let code = entry.parent();
    let package = match code {
        Some(dir) if dir.file_name().is_some_and(|n| n == "dist") => dir.parent(),
        other => other,
    };
    if let Some(dir) = package {
        roots.push(dir.to_path_buf());
    }
    if let Some(modules) = entry
        .ancestors()
        .map(|dir| dir.join("node_modules"))
        .find(|dir| dir.join("node-llama-cpp").is_dir())
    {
        roots.push(modules);
    }
    if let Some(dir) = models_dir {
        roots.push(dir.to_path_buf());
    }
    #[cfg(windows)]
    {
        custodian_winsec::appcontainer::distinct_roots(&roots)
    }
    #[cfg(not(windows))]
    {
        roots
    }
}

/// `node` resolved to a full path: `CUSTODIAN_NODE` if it is one, otherwise
/// the first `node.exe` (or `node`) on the PATH.
pub fn resolve_node(name: &str) -> Option<PathBuf> {
    let given = Path::new(name);
    if given.is_absolute() {
        return given.is_file().then(|| given.to_path_buf());
    }
    let file = if cfg!(windows) && given.extension().is_none() {
        format!("{name}.exe")
    } else {
        name.to_string()
    };
    std::env::split_paths(&std::env::var_os("PATH")?)
        .map(|dir| dir.join(&file))
        .find(|path| path.is_file())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{json, Value};
    use std::thread;

    fn no_tools(_: &str, _: &Value) -> String {
        panic!("no tool was expected")
    }

    #[test]
    fn runs_the_tools_the_model_asks_for() {
        let (to_client_r, mut to_client_w) = std::io::pipe().unwrap();
        let (mut to_agent_r, to_agent_w) = std::io::pipe().unwrap();
        let key = b"test-key".to_vec();
        let agent_key = key.clone();
        thread::spawn(move || {
            let request: Value = read_frame(&mut to_agent_r, &agent_key).unwrap();
            let id = request["id"].as_u64().unwrap();
            let call = json!({"type": "tool", "id": id, "call": 1, "name": "find_files",
                              "args": {"name": "resume", "folders": []}});
            write_frame(&mut to_client_w, &call, &agent_key).unwrap();
            let result: Value = read_frame(&mut to_agent_r, &agent_key).unwrap();
            assert_eq!(result["type"], "tool_result");
            assert_eq!(result["call"], 1);
            let text = format!("It is at {}", result["result"].as_str().unwrap());
            for frame in [
                json!({"type": "token", "id": id, "text": text}),
                json!({"type": "done", "id": id}),
            ] {
                write_frame(&mut to_client_w, &frame, &agent_key).unwrap();
            }
        });
        let mut conn = Connection::new(to_client_r, to_agent_w, key);
        let mut text = String::new();
        let mut asked = Vec::new();
        conn.chat(
            "where is my resume?",
            &AtomicBool::new(false),
            &mut |t| text.push_str(t),
            &mut |name, args| {
                asked.push((name.to_string(), args.clone()));
                "Documents\\resume.pdf".to_string()
            },
        )
        .unwrap();
        assert_eq!(text, "It is at Documents\\resume.pdf");
        assert_eq!(asked.len(), 1);
        assert_eq!(asked[0].0, "find_files");
        assert_eq!(asked[0].1["name"], "resume");
    }

    /// A fake agent on the other end of two OS pipes.
    fn fake_agent(
        reply: impl FnOnce(u64, &str) -> Vec<Value> + Send + 'static,
    ) -> Connection<std::io::PipeReader, std::io::PipeWriter> {
        let (to_client_r, mut to_client_w) = std::io::pipe().unwrap();
        let (mut to_agent_r, to_agent_w) = std::io::pipe().unwrap();
        let key = b"test-key".to_vec();
        let agent_key = key.clone();
        thread::spawn(move || {
            let request: Value = read_frame(&mut to_agent_r, &agent_key).unwrap();
            let id = request["id"].as_u64().unwrap();
            for frame in reply(id, request["message"].as_str().unwrap_or_default()) {
                write_frame(&mut to_client_w, &frame, &agent_key).unwrap();
            }
        });
        Connection::new(to_client_r, to_agent_w, key)
    }

    #[test]
    fn streams_tokens_until_done() {
        let mut conn = fake_agent(|id, message| {
            vec![
                json!({"type": "token", "id": id, "text": "You said: "}),
                json!({"type": "token", "id": id, "text": message}),
                json!({"type": "done", "id": id}),
            ]
        });
        let mut text = String::new();
        conn.chat(
            "hello",
            &AtomicBool::new(false),
            &mut |t| text.push_str(t),
            &mut no_tools,
        )
        .unwrap();
        assert_eq!(text, "You said: hello");
    }

    #[test]
    fn stop_tells_the_agent_and_hides_the_rest() {
        let (to_client_r, mut to_client_w) = std::io::pipe().unwrap();
        let (mut to_agent_r, to_agent_w) = std::io::pipe().unwrap();
        let key = b"test-key".to_vec();
        let agent_key = key.clone();
        let agent = thread::spawn(move || {
            let request: Value = read_frame(&mut to_agent_r, &agent_key).unwrap();
            let id = request["id"].as_u64().unwrap();
            let token = |text: &str| json!({"type": "token", "id": id, "text": text});
            write_frame(&mut to_client_w, &token("first "), &agent_key).unwrap();
            write_frame(&mut to_client_w, &token("second "), &agent_key).unwrap();
            // The client answers the second token with a cancel frame.
            let cancel: Value = read_frame(&mut to_agent_r, &agent_key).unwrap();
            assert_eq!(cancel, json!({"type": "cancel", "id": id}));
            write_frame(&mut to_client_w, &token("ignored"), &agent_key).unwrap();
            let tool =
                json!({"type": "tool", "id": id, "call": 1, "name": "find_files", "args": {}});
            write_frame(&mut to_client_w, &tool, &agent_key).unwrap();
            let result: Value = read_frame(&mut to_agent_r, &agent_key).unwrap();
            assert_eq!(result["result"], STOPPED);
            write_frame(
                &mut to_client_w,
                &json!({"type": "done", "id": id}),
                &agent_key,
            )
            .unwrap();
        });
        let mut conn = Connection::new(to_client_r, to_agent_w, key);
        let cancel = AtomicBool::new(false);
        let mut text = String::new();
        conn.chat(
            "tell me a long story",
            &cancel,
            &mut |t| {
                text.push_str(t);
                cancel.store(true, Ordering::Relaxed); // Stop after the first words.
            },
            &mut no_tools,
        )
        .unwrap();
        agent.join().unwrap();
        assert_eq!(text, "first ");
    }

    #[test]
    fn pings_until_the_matching_pong() {
        let mut conn = fake_agent(|id, _| {
            vec![
                json!({"type": "pong", "id": id + 7, "ready": true}),
                json!({"type": "pong", "id": id, "ready": false, "error": "EPERM: model"}),
            ]
        });
        assert_eq!(conn.ping().unwrap(), Some("EPERM: model".into()));
    }

    #[test]
    fn finds_what_the_sandbox_may_read() {
        let dir = tempfile::tempdir().unwrap();
        let repo = dir.path();
        std::fs::create_dir_all(repo.join("node_modules/node-llama-cpp")).unwrap();
        std::fs::create_dir_all(repo.join("services/agent/dist")).unwrap();
        let node = repo.join("nodejs").join("node.exe");
        let entry = repo.join("services/agent/dist/server.js");
        let models = repo.join("models");
        let roots = sandbox_read_roots(&node, &entry, Some(&models));
        assert!(roots.contains(&repo.join("nodejs")));
        assert!(roots.contains(&repo.join("services/agent")));
        assert!(roots.contains(&repo.join("node_modules")));
        assert!(roots.contains(&models));
        assert!(
            !roots.contains(&repo.to_path_buf()),
            "never the whole checkout"
        );
    }

    #[test]
    fn reports_model_errors() {
        let mut conn = fake_agent(|id, _| {
            vec![json!({"type": "error", "id": id, "error": "no model installed"})]
        });
        let err = conn
            .chat("hi", &AtomicBool::new(false), &mut |_| {}, &mut no_tools)
            .unwrap_err();
        assert!(matches!(err, AgentError::Model(m) if m == "no model installed"));
    }

    #[test]
    fn ignores_replies_for_other_requests() {
        let mut conn = fake_agent(|id, _| {
            vec![
                json!({"type": "token", "id": id + 100, "text": "stale"}),
                json!({"type": "token", "id": id, "text": "fresh"}),
                json!({"type": "done", "id": id}),
            ]
        });
        let mut text = String::new();
        conn.chat(
            "x",
            &AtomicBool::new(false),
            &mut |t| text.push_str(t),
            &mut no_tools,
        )
        .unwrap();
        assert_eq!(text, "fresh");
    }

    #[test]
    fn rejects_frames_signed_with_another_key() {
        let (to_client_r, mut to_client_w) = std::io::pipe().unwrap();
        let (_to_agent_r, to_agent_w) = std::io::pipe().unwrap();
        thread::spawn(move || {
            write_frame(
                &mut to_client_w,
                &json!({"type": "done", "id": 1}),
                b"attacker",
            )
            .unwrap();
        });
        let mut conn = Connection::new(to_client_r, to_agent_w, b"real".to_vec());
        assert!(matches!(
            conn.chat("x", &AtomicBool::new(false), &mut |_| {}, &mut no_tools),
            Err(AgentError::Ipc(_))
        ));
    }

    #[test]
    fn silent_agent_stop_aborts_without_hanging() {
        let (to_client_r, _to_client_w) = std::io::pipe().unwrap();
        let (_to_agent_r, to_agent_w) = std::io::pipe().unwrap();
        let key = b"test-key".to_vec();
        let mut conn = Connection::new(to_client_r, to_agent_w, key);
        let cancel = AtomicBool::new(true); // Stop requested immediately
        let err = conn
            .chat("hello", &cancel, &mut |_| {}, &mut no_tools)
            .unwrap_err();
        assert!(matches!(err, AgentError::Timeout(msg) if msg.contains("cancel within deadline")));
    }

    /// Runs the real agent and model inside its AppContainer, as Custodian
    /// does: it must be able to load the model there and answer. Needs
    /// `npm run build` and a pinned chat model in `models/`; run with
    /// `cargo test -p custodian-desktop -- --ignored real_model_in_sandbox --nocapture`.
    #[cfg(windows)]
    #[test]
    #[ignore = "needs the built agent and a local model"]
    fn real_model_in_sandbox_answers() {
        use custodian_winsec::appcontainer::Sandbox;
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..");
        let entry = std::fs::canonicalize(root.join("services/agent/dist/server.js")).unwrap();
        let models = std::fs::canonicalize(root.join("models")).unwrap();
        let node = resolve_node("node").expect("node on the PATH");
        let sandbox = Sandbox::open("custodian.agent").unwrap();
        let start = std::time::Instant::now();
        for dir in sandbox_read_roots(&node, &entry, Some(&models)) {
            let changed = sandbox.grant_read(&dir).unwrap();
            eprintln!("read access for {} (changed: {changed})", dir.display());
        }
        eprintln!("grants took {:?}", start.elapsed());
        let mut agent =
            AgentProcess::spawn_sandboxed(&sandbox, &node, &entry, Some(&models)).unwrap();
        assert!(agent.sandboxed());
        assert_eq!(
            agent.ping().unwrap(),
            None,
            "the model can be used in the sandbox"
        );
        let mut reply = String::new();
        agent
            .chat(
                "Reply with one short sentence: what is a duplicate file?",
                &AtomicBool::new(false),
                &mut |t| reply.push_str(t),
                &mut |_, _| "No tools in this test.".to_string(),
            )
            .unwrap();
        eprintln!("sandboxed model reply: {reply}");
        assert!(
            reply.to_lowercase().contains("copy")
                || reply.to_lowercase().contains("identical")
                || reply.to_lowercase().contains("same"),
            "{reply}"
        );
        let mut calls = Vec::new();
        let mut answer = String::new();
        agent
            .chat(
                "How much free disk space do I have on my C drive?",
                &AtomicBool::new(false),
                &mut |text| answer.push_str(text),
                &mut |name, _| {
                    calls.push(name.to_string());
                    "Shown to the person: C: has 41 GB free of 476 GB.".to_string()
                },
            )
            .unwrap();
        assert!(
            calls.iter().any(|name| name == "check_disk_space"),
            "{calls:?}: {answer}"
        );
    }

    #[cfg(windows)]
    #[test]
    #[ignore = "needs an isolated staged runtime in CUSTODIAN_RUNTIME_TEST_ROOT"]
    fn installed_runtime_answers_in_sandbox_without_checkout_dependencies() {
        use custodian_winsec::appcontainer::Sandbox;
        let root = PathBuf::from(
            std::env::var_os("CUSTODIAN_RUNTIME_TEST_ROOT").expect("set the isolated runtime root"),
        );
        let node = root.join("bin/node.exe");
        let entry = root.join("agent/server.js");
        let models = root.join("models");
        let sandbox = Sandbox::open("custodian.runtime-verification").unwrap();
        for dir in sandbox_read_roots(&node, &entry, Some(&models)) {
            sandbox.grant_read(&dir).unwrap();
        }
        let mut agent =
            AgentProcess::spawn_sandboxed(&sandbox, &node, &entry, Some(&models)).unwrap();
        assert_eq!(agent.ping().unwrap(), None);
        let mut reply = String::new();
        agent
            .chat(
                "Reply with one short sentence: what is a duplicate file?",
                &AtomicBool::new(false),
                &mut |text| reply.push_str(text),
                &mut |_, _| "No tools in this test.".into(),
            )
            .unwrap();
        let answer = reply.to_lowercase();
        assert!(
            answer.contains("copy") || answer.contains("identical") || answer.contains("same"),
            "{reply}"
        );
        assert!(
            !answer.contains("not have enough") && !answer.contains("don't have enough"),
            "{reply}"
        );
        eprintln!("Installed isolated runtime reply: {reply}");
        assert!(
            (answer.contains("identical")
                || answer.contains("same content")
                || answer.contains("same data")
                || answer.contains("exact copy"))
                && !answer.contains("slight differences")
                && !answer.contains("different content"),
            "The model must explain content identity, not merely mention a copy: {reply}"
        );
        let mut calls = Vec::new();
        let mut answer = String::new();
        agent
            .chat(
                "How much free disk space do I have on my C drive?",
                &AtomicBool::new(false),
                &mut |text| answer.push_str(text),
                &mut |name, _| {
                    calls.push(name.to_string());
                    "Shown to the person: C: has 41 GB free of 476 GB.".into()
                },
            )
            .unwrap();
        assert!(
            calls.iter().any(|name| name == "check_disk_space"),
            "{calls:?}: {answer}"
        );
    }

    /// Chats with the real agent and model under the job limits. Needs
    /// `npm run build` and a pinned chat model in `models/`; run with
    /// `cargo test -p custodian-desktop -- --ignored real_model`.
    #[test]
    #[ignore = "needs the built agent and a local model"]
    fn real_model_answers_under_job_limits() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..");
        let entry = root.join("services/agent/dist/server.js");
        let models = root.join("models");
        let mut agent = AgentProcess::spawn("node", &entry, Some(&models)).unwrap();
        assert!(agent._job.is_some(), "the job must be in place");
        assert_eq!(agent.ping().unwrap(), None, "the model can be used");
        let mut reply = String::new();
        agent
            .chat(
                "Reply with one short sentence: what is a duplicate file?",
                &AtomicBool::new(false),
                &mut |t| reply.push_str(t),
                &mut |_, _| "No tools in this test.".to_string(),
            )
            .unwrap();
        assert!(reply.to_lowercase().contains("file"), "{reply}");
        eprintln!("model reply: {reply}");
    }
}
