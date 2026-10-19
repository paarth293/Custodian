//! Trusted native worker protocol, not a filesystem/network sandbox. No model
//! calls, shell, input injection, screenshots or qualified app adapters yet.
use custodian_core::app_worker::{
    Operation, Reply, Request, WorkerResult, FRAME_LIMIT, PROTOCOL_VERSION,
};
use custodian_ipc::{read_frame_limited, write_frame};
use std::io::{self, Read, Write};

pub fn serve_once(mut input: impl Read, mut output: impl Write) -> io::Result<()> {
    let invalid = || io::Error::new(io::ErrorKind::InvalidData, "invalid worker protocol");
    let mut key = [0; 32];
    input.read_exact(&mut key)?;
    let request: Request =
        read_frame_limited(&mut input, &key, FRAME_LIMIT).map_err(|_| invalid())?;
    if request.version != PROTOCOL_VERSION
        || request.nonce.len() != 36
        || !request
            .nonce
            .bytes()
            .all(|b| b.is_ascii_hexdigit() || b == b'-')
    {
        return Err(invalid());
    }
    // The supervisor closes stdin after one frame. Refuse extra commands.
    let mut extra = [0];
    if !matches!(&request.operation, Operation::StartExcel { .. }) && input.read(&mut extra)? != 0 {
        return Err(invalid());
    }
    let result = match request.operation {
        Operation::StartExcel { config } => {
            return excel::serve(config, request.nonce, key, input, output);
        }
        Operation::ProbeSelf | Operation::ProbeOwnedWindow => {
            let owned_window = matches!(request.operation, Operation::ProbeOwnedWindow);
            if owned_window {
                custodian_winsec::apps::probe_owned_window()?;
            }
            let identity = custodian_winsec::apps::process_identity(std::process::id())
                .ok_or_else(|| io::Error::other("native worker identity unavailable"))?;
            WorkerResult::Probe {
                pid: identity.pid,
                creation_time: identity.creation_time,
                owned_window_checked: owned_window,
            }
        }
        // Never echo a claimed target as if it were measured. Qualification and
        // private-document enforcement must exist before these branches change.
        Operation::Remeasure { .. } | Operation::Execute { .. } => WorkerResult::Unavailable,
    };
    let reply = Reply {
        version: PROTOCOL_VERSION,
        nonce: request.nonce,
        result,
    };
    write_frame(&mut output, &reply, &key).map_err(|_| invalid())
}

#[cfg(windows)]
mod excel {
    use super::*;
    use custodian_core::{
        app_adapter::validate_cell_edits,
        excel_protocol::{Action, Command, Config, Data, Reply as ExcelReply},
        workbook,
    };
    use custodian_winsec::excel::ExcelSession;
    use std::{fs::OpenOptions, os::windows::fs::OpenOptionsExt};

    pub fn serve(
        config: Config,
        nonce: String,
        mut key: [u8; 32],
        mut input: impl Read,
        mut output: impl Write,
    ) -> io::Result<()> {
        let invalid = || io::Error::other("Excel worker refused; reconcile owned task state");
        if config.existing_pids.len() > 65536
            || !config.source.is_absolute()
            || !config.output.is_absolute()
            || config.source.parent() != config.output.parent()
            || config.source == config.output
            || config.source.file_name().and_then(|s| s.to_str()) != Some("source.xlsx")
            || config.output.file_name().and_then(|s| s.to_str()) != Some("output.xlsx")
            || config.output.try_exists()?
        {
            return Err(invalid());
        }
        let mut source = OpenOptions::new()
            .read(true)
            .share_mode(1)
            .open(&config.source)?;
        let metadata = source.metadata()?;
        if !metadata.is_file() || metadata.len() > 8 * 1024 * 1024 {
            return Err(invalid());
        }
        let mut bytes = vec![];
        Read::by_ref(&mut source)
            .take(8 * 1024 * 1024 + 1)
            .read_to_end(&mut bytes)?;
        let admitted = workbook::inspect(&bytes).map_err(|_| invalid())?;
        workbook::recalculate(&admitted).map_err(|_| invalid())?;
        let mut excel = ExcelSession::start(&config.existing_pids)?;
        let identity = excel.identity();
        let hello = Reply {
            version: PROTOCOL_VERSION,
            nonce,
            result: WorkerResult::Excel {
                reply: ExcelReply {
                    session: config.session,
                    sequence: 0,
                    result: Data::Started {
                        pid: identity.pid,
                        birth: identity.creation_time,
                        window: excel.window()?,
                    },
                },
            },
        };
        write_frame(&mut output, &hello, &key).map_err(|_| invalid())?;
        output.flush()?;
        let mut opened = false;
        let mut saved = false;
        for sequence in 1..=128 {
            let command: Command =
                read_frame_limited(&mut input, &key, FRAME_LIMIT).map_err(|_| invalid())?;
            if command.session != config.session || command.sequence != sequence {
                return Err(invalid());
            }
            let closing = matches!(&command.operation, Action::Close { .. });
            let operation = (|| -> io::Result<Data> {
                match command.operation {
                    Action::Open if !opened => {
                        excel.open_task_copy(&config.source)?;
                        opened = true;
                        Ok(Data::Opened {
                            window: excel.window()?,
                        })
                    }
                    Action::Metadata if opened => {
                        let (window, saved) = excel.metadata()?;
                        Ok(Data::Metadata { window, saved })
                    }
                    Action::Inspect if opened => {
                        let snapshot = excel.snapshot()?;
                        if !saved && snapshot != admitted {
                            return Err(invalid());
                        }
                        Ok(Data::Snapshot { snapshot })
                    }
                    Action::Edit {
                        expected,
                        sheet_id,
                        cells,
                    } if opened && !saved => {
                        if sheet_id != format!("{}:sheet:1", config.session) {
                            return Err(invalid());
                        }
                        validate_cell_edits(&sheet_id, &cells).map_err(|_| invalid())?;
                        let mut snapshot = expected;
                        workbook::recalculate(&snapshot).map_err(|_| invalid())?;
                        for cell in cells {
                            snapshot = excel.edit_cell(
                                &snapshot,
                                cell.row,
                                cell.col,
                                &cell.value.native().map_err(|_| invalid())?,
                            )?;
                        }
                        Ok(Data::Snapshot { snapshot })
                    }
                    Action::Save { expected } if opened && !saved => {
                        excel.save_new(&config.output, &expected)?;
                        saved = true;
                        Ok(Data::Saved)
                    }
                    Action::Close { expected } if opened && saved => {
                        excel.close(&expected)?;
                        Ok(Data::Closed)
                    }
                    _ => Err(invalid()),
                }
            })();
            let failed = operation.is_err();
            let reply = ExcelReply {
                session: config.session,
                sequence,
                result: operation.unwrap_or(Data::Refused),
            };
            write_frame(&mut output, &reply, &key).map_err(|_| invalid())?;
            output.flush()?;
            if closing || failed {
                key.fill(0);
                return Ok(());
            }
        }
        key.fill(0);
        Err(invalid())
    }
}

#[cfg(not(windows))]
mod excel {
    use super::*;
    pub fn serve(
        _: custodian_core::excel_protocol::Config,
        _: String,
        _: [u8; 32],
        _: impl Read,
        _: impl Write,
    ) -> io::Result<()> {
        Err(io::Error::other("Excel unavailable on this platform"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn authentication_and_typed_schema_fail_before_native_operations() {
        let mut wire = vec![1u8; 32];
        write_frame(&mut wire, &serde_json::json!({"version":1,"nonce":"00000000-0000-4000-8000-000000000000","operation":{"kind":"shell","command":"anything"}}), &[1;32]).unwrap();
        let mut out = Vec::new();
        assert!(serve_once(wire.as_slice(), &mut out).is_err());
        assert!(out.is_empty());
        let mut wire = vec![1u8; 32];
        write_frame(&mut wire, &serde_json::json!({"version":1}), &[2; 32]).unwrap();
        assert!(serve_once(wire.as_slice(), &mut out).is_err());
        assert!(out.is_empty());
    }
}
