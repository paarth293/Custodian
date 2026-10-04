//! Client for the agent process (`services/agent`), which runs the local
//! language model. Messages are HMAC-authenticated frames (`custodian-ipc`)
//! over the child's stdin/stdout, with a fresh random key per launch.

use custodian_ipc::{read_frame, write_frame, IpcError};
use rand::RngCore;
use serde::{Deserialize, Serialize};
use std::io::{BufReader, BufWriter, Read, Write};
use std::path::PathBuf;
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum AgentError {
    #[error("the agent could not be started: {0}")]
    Spawn(String),
    #[error("lost contact with the agent: {0}")]
    Ipc(#[from] IpcError),
    #[error("{0}")]
    Model(String),
}

#[derive(Serialize)]
#[serde(tag = "type", rename_all = "lowercase")]
enum Request<'a> {
    Chat { id: u64, message: &'a str },
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
enum Reply {
    Token { id: u64, text: String },
    Done { id: u64 },
    Error { id: u64, error: String },
}

/// One request/response conversation channel to an agent.
pub struct Connection<R: Read, W: Write> {
    reader: BufReader<R>,
    writer: BufWriter<W>,
    key: Vec<u8>,
    next_id: u64,
}

impl<R: Read, W: Write> Connection<R, W> {
    pub fn new(reader: R, writer: W, key: Vec<u8>) -> Self {
        Self {
            reader: BufReader::new(reader),
            writer: BufWriter::new(writer),
            key,
            next_id: 1,
        }
    }

    /// Sends a message and streams the reply's text chunks to `on_text`.
    pub fn chat(&mut self, message: &str, on_text: &mut dyn FnMut(&str)) -> Result<(), AgentError> {
        let id = self.next_id;
        self.next_id += 1;
        write_frame(&mut self.writer, &Request::Chat { id, message }, &self.key)?;
        loop {
            match read_frame::<Reply, _>(&mut self.reader, &self.key)? {
                Reply::Token { id: rid, text } if rid == id => on_text(&text),
                Reply::Done { id: rid } if rid == id => return Ok(()),
                Reply::Error { id: rid, error } if rid == id => {
                    return Err(AgentError::Model(error))
                }
                // Replies to an earlier, abandoned request are ignored.
                _ => {}
            }
        }
    }
}

/// A running agent process.
pub struct AgentProcess {
    child: Child,
    connection: Connection<ChildStdout, ChildStdin>,
}

impl AgentProcess {
    /// Starts `node <entry>` with a fresh IPC key.
    pub fn spawn(
        node: &str,
        entry: &PathBuf,
        models_dir: Option<&PathBuf>,
    ) -> Result<Self, AgentError> {
        let mut key = vec![0u8; 32];
        rand::rngs::OsRng.fill_bytes(&mut key);
        let key_hex: String = key.iter().map(|b| format!("{b:02x}")).collect();

        let mut command = Command::new(node);
        command
            .arg(entry)
            .env("CUSTODIAN_IPC_KEY", &key_hex)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit());
        if let Some(dir) = models_dir {
            command.env("CUSTODIAN_MODELS_DIR", dir);
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
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| AgentError::Spawn("no stdout".into()))?;
        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| AgentError::Spawn("no stdin".into()))?;
        // The key is hex text on the agent side, so authenticate with its bytes.
        let connection = Connection::new(stdout, stdin, key_hex.into_bytes());
        Ok(Self { child, connection })
    }

    pub fn chat(&mut self, message: &str, on_text: &mut dyn FnMut(&str)) -> Result<(), AgentError> {
        self.connection.chat(message, on_text)
    }

    pub fn is_alive(&mut self) -> bool {
        matches!(self.child.try_wait(), Ok(None))
    }
}

impl Drop for AgentProcess {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{json, Value};
    use std::thread;

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
            for frame in reply(id, request["message"].as_str().unwrap()) {
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
        conn.chat("hello", &mut |t| text.push_str(t)).unwrap();
        assert_eq!(text, "You said: hello");
    }

    #[test]
    fn reports_model_errors() {
        let mut conn = fake_agent(|id, _| {
            vec![json!({"type": "error", "id": id, "error": "no model installed"})]
        });
        let err = conn.chat("hi", &mut |_| {}).unwrap_err();
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
        conn.chat("x", &mut |t| text.push_str(t)).unwrap();
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
            conn.chat("x", &mut |_| {}),
            Err(AgentError::Ipc(_))
        ));
    }
}
