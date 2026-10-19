#[cfg(windows)]
mod native {
    //! Fault peer built ONLY as an explicitly requested example. Not a production
    //! adapter, not installed/bundled, and never selected by model/user requests.
    use custodian_core::app_worker::{
        Operation, Reply, Request, WorkerResult, FRAME_LIMIT, PROTOCOL_VERSION,
    };
    use custodian_ipc::{read_frame_limited, write_frame};
    use std::io::{Read, Write};
    use std::time::Duration;

    pub fn run() {
        if std::env::args().nth(1).as_deref() == Some("--sentinel") {
            std::thread::sleep(Duration::from_secs(120));
            return;
        }
        let exe = std::env::current_exe().unwrap();
        let directory = exe.parent().unwrap();
        let mode = directory.file_name().unwrap().to_string_lossy();
        let own_identity = custodian_winsec::apps::process_identity(std::process::id()).unwrap();
        let mut pid_log = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(directory.join("worker-pids.txt"))
            .unwrap();
        writeln!(
            pid_log,
            "{} {}",
            own_identity.pid, own_identity.creation_time
        )
        .unwrap();
        pid_log.sync_all().unwrap();
        if mode == "no-read" {
            std::thread::sleep(Duration::from_secs(120));
            return;
        }
        let mut input = std::io::stdin().lock();
        let mut output = std::io::stdout().lock();
        let mut key = [0; 32];
        input.read_exact(&mut key).unwrap();
        let request: Request = read_frame_limited(&mut input, &key, FRAME_LIMIT).unwrap();
        if let Operation::StartExcel { config } = request.operation {
            session_fixture(config, request.nonce, key, input, output, directory, &mode);
            return;
        }
        let mut extra = [0];
        assert_eq!(input.read(&mut extra).unwrap(), 0);
        match mode.as_ref() {
            "exit-259" => std::process::exit(259),
            "hang" => {
                std::thread::sleep(Duration::from_secs(120));
                return;
            }
            "partial" => {
                output.write_all(b"CUST").unwrap();
                output.write_all(&1024u32.to_le_bytes()).unwrap();
                output.flush().unwrap();
                std::thread::sleep(Duration::from_secs(120));
                return;
            }
            "oversize" => {
                output.write_all(b"CUST").unwrap();
                output.write_all(&u32::MAX.to_le_bytes()).unwrap();
                return;
            }
            "stderr-flood" => {
                std::io::stderr().write_all(&[b'x'; 8192]).unwrap();
            }
            "memory" => {
                // Fallible allocation avoids invoking Rust's abort/WER path,
                // which can wait for OS crash reporting instead of exiting.
                let mut data = Vec::<u8>::new();
                if data.try_reserve_exact(768 * 1024 * 1024).is_ok() {
                    std::process::exit(9); // Job failed to reject the allocation.
                }
            }
            "child-limit" => {
                use std::os::windows::process::CommandExt;
                if let Ok(mut child) = std::process::Command::new(&exe)
                    .arg("--sentinel")
                    .creation_flags(0x08000000)
                    .stdin(std::process::Stdio::null())
                    .stdout(std::process::Stdio::null())
                    .stderr(std::process::Stdio::null())
                    .spawn()
                {
                    for _ in 0..100 {
                        if child.try_wait().unwrap().is_some() {
                            break;
                        }
                        std::thread::sleep(Duration::from_millis(10));
                    }
                    if child.try_wait().unwrap().is_none() {
                        child.kill().unwrap();
                        child.wait().unwrap();
                        std::process::exit(9);
                    }
                }
            }
            _ => {}
        }
        let identity = custodian_winsec::apps::process_identity(std::process::id()).unwrap();
        let result = match request.operation {
            Operation::StartExcel { .. } => WorkerResult::Unavailable,
            Operation::ProbeSelf | Operation::ProbeOwnedWindow => WorkerResult::Probe {
                pid: identity.pid,
                creation_time: identity.creation_time,
                owned_window_checked: false,
            },
            Operation::Remeasure { request } => WorkerResult::Measured { request },
            Operation::Execute { request } => {
                // A real synthetic side effect for crash/recovery tests, never an app.
                std::fs::write(directory.join("synthetic-effect.txt"), b"fixture effect").unwrap();
                if mode == "crash-effect" {
                    std::process::exit(8);
                }
                WorkerResult::Receipt {
                    receipt: Box::new(custodian_core::app_adapter::ApplicationReceipt {
                        identity: request.identity,
                        document_id: request.document_id,
                        document_name: request.document_name,
                        action: request.action,
                        verified: true,
                        detail: "synthetic fixture effect only".into(),
                    }),
                }
            }
        };
        let reply = Reply {
            version: if mode == "wrong-version" {
                99
            } else {
                PROTOCOL_VERSION
            },
            nonce: if mode == "wrong-nonce" {
                "00000000-0000-4000-8000-000000000000".into()
            } else {
                request.nonce
            },
            result,
        };
        let mut wire = Vec::new();
        write_frame(&mut wire, &reply, &key).unwrap();
        if mode == "bad-mac" {
            wire[8] ^= 1;
        }
        output.write_all(&wire).unwrap();
        output.flush().unwrap();
        if mode == "trailing-frame" {
            output.write_all(b"unexpected").unwrap();
            output.flush().unwrap();
        }
        if mode == "reply-hang" {
            std::thread::sleep(Duration::from_secs(120));
        }
    }

    fn session_fixture(
        config: custodian_core::excel_protocol::Config,
        nonce: String,
        mut key: [u8; 32],
        mut input: impl Read,
        mut output: impl Write,
        directory: &std::path::Path,
        mode: &str,
    ) {
        use custodian_core::excel_protocol::{Command, Data, Reply as SessionReply};
        if mode == "session-start-hang" {
            std::thread::sleep(Duration::from_secs(120));
            return;
        }
        let identity = custodian_winsec::apps::process_identity(std::process::id()).unwrap();
        write_frame(
            &mut output,
            &Reply {
                version: PROTOCOL_VERSION,
                nonce,
                result: WorkerResult::Excel {
                    reply: SessionReply {
                        session: config.session,
                        sequence: 0,
                        result: Data::Started {
                            pid: identity.pid,
                            birth: identity.creation_time,
                            window: 0,
                        },
                    },
                },
            },
            &key,
        )
        .unwrap();
        output.flush().unwrap();
        let command: Command = read_frame_limited(&mut input, &key, FRAME_LIMIT).unwrap();
        std::fs::write(directory.join("command-read.txt"), b"one command").unwrap();
        if mode == "session-hang" || mode == "session-stop" {
            std::thread::sleep(Duration::from_secs(120));
            return;
        }
        if mode == "session-partial" {
            output.write_all(b"CUST\0\x04\0\0").unwrap();
            output.flush().unwrap();
            std::thread::sleep(Duration::from_secs(120));
            return;
        }
        if mode == "session-oversize" {
            output.write_all(b"CUST").unwrap();
            output.write_all(&u32::MAX.to_le_bytes()).unwrap();
            return;
        }
        if mode == "session-bad-mac" {
            key[0] ^= 1;
        }
        let reply = SessionReply {
            session: if mode == "session-wrong-session" {
                uuid::Uuid::nil()
            } else {
                config.session
            },
            sequence: command.sequence + u32::from(mode == "session-wrong-sequence"),
            result: if mode == "session-refused" {
                Data::Refused
            } else {
                Data::Metadata {
                    window: 0,
                    saved: true,
                }
            },
        };
        write_frame(&mut output, &reply, &key).unwrap();
        output.flush().unwrap();
    }
}

#[cfg(windows)]
fn main() {
    native::run();
}

#[cfg(not(windows))]
fn main() {}
