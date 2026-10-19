// Included inside the Windows worker module to share its pinned program and
// cleanup guard. The protocol is available only to the native Excel adapter.
pub struct WorkerSession<'a> {
    guard: ChildGuard<'a>,
    _lease: std::sync::MutexGuard<'a, ()>,
    input: Option<std::fs::File>,
    output: std::fs::File,
    errors: std::fs::File,
    key: [u8; 32],
    session: uuid::Uuid,
    sequence: u32,
    buffer: Vec<u8>,
    error_bytes: usize,
    started: std::time::Instant,
    unusable: bool,
}
impl WorkerProgram {
    pub(crate) fn start_excel(
        &self,
        config: crate::excel_protocol::Config,
        timeout: std::time::Duration,
        check: impl Fn() -> Result<(), WorkerFailure>,
    ) -> Result<(WorkerSession<'_>, crate::excel_protocol::Data), WorkerFailure> {
        if timeout.is_zero() || timeout > Duration::from_secs(120) {
            return Err(WorkerFailure::InvalidConfiguration);
        }
        let startup = Instant::now();
        check()?;
        let lease = self.active.try_lock().map_err(|_| WorkerFailure::Busy)?;
        if self.poisoned.load(Ordering::Acquire) { return Err(WorkerFailure::Cleanup); }
        let environment = ["SystemRoot", "windir", "TEMP", "TMP", "USERPROFILE", "APPDATA", "LOCALAPPDATA"]
            .into_iter().filter_map(|name| std::env::var_os(name).map(|value| (name, value)))
            .collect::<Vec<_>>();
        let env = environment.iter().map(|(name, value)| (*name, value.as_os_str())).collect::<Vec<_>>();
        let child = spawn_trusted_worker(&self.path, &env, self.path.parent()).map_err(|_| WorkerFailure::Start)?;
        let mut guard = ChildGuard { child, job: None, poisoned: &self.poisoned, cleanup_checked: false };
        let job = Job::new(JobLimits { memory_mb: Some(512), cpu_percent: Some(25),
            below_normal_priority: true, process_limit: Some(1) }).map_err(|_| WorkerFailure::Start)?;
        job.assign_raw(guard.child.raw_process()).map_err(|_| WorkerFailure::Start)?;
        guard.job = Some(job);
        check()?;
        let input = guard.child.stdin.take().ok_or(WorkerFailure::Start)?;
        let output = guard.child.stdout.take().ok_or(WorkerFailure::Start)?;
        let errors = guard.child.stderr.take().ok_or(WorkerFailure::Start)?;
        guard.child.resume().map_err(|_| WorkerFailure::Start)?;
        let mut key = [0;32]; rand::rngs::OsRng.fill_bytes(&mut key);
        let session = config.session;
        let request = Request { version: PROTOCOL_VERSION, nonce: uuid::Uuid::new_v4().to_string(),
            operation: Operation::StartExcel { config } };
        let mut frame = key.to_vec();
        write_frame(&mut frame, &request, &key).map_err(|_| WorkerFailure::Protocol)?;
        let mut transport = WorkerSession { guard, _lease: lease, input: Some(input), output, errors,
            key, session, sequence: 0, buffer: vec![], error_bytes: 0,
            started: Instant::now(), unusable: false };
        transport.send(frame, timeout.checked_sub(startup.elapsed()).ok_or(WorkerFailure::Timeout)?, &check)?;
        let reply: Reply = transport.receive(timeout.checked_sub(startup.elapsed()).ok_or(WorkerFailure::Timeout)?, &check)?;
        match reply.result {
            WorkerResult::Excel { reply: hello } if reply.version == PROTOCOL_VERSION
                && reply.nonce == request.nonce && hello.session == session && hello.sequence == 0 =>
                Ok((transport, hello.result)),
            _ => Err(WorkerFailure::Protocol),
        }
    }
}
impl WorkerSession<'_> {
    fn live(&self, start: Instant, timeout: Duration, check: &impl Fn() -> Result<(), WorkerFailure>) -> Result<(), WorkerFailure> {
        check()?;
        if self.unusable { return Err(WorkerFailure::Cleanup); }
        if start.elapsed() >= timeout || self.started.elapsed() >= Duration::from_secs(1800) {
            return Err(WorkerFailure::Timeout);
        }
        Ok(())
    }
    fn send(&mut self, frame: Vec<u8>, timeout: Duration, check: &impl Fn() -> Result<(), WorkerFailure>) -> Result<(), WorkerFailure> {
        self.live(Instant::now(), timeout, check)?;
        if frame.len() > FRAME_LIMIT as usize + 72 { return Err(WorkerFailure::Protocol); }
        let mut input = self.input.take().ok_or(WorkerFailure::Cleanup)?;
        let (sender, receiver) = mpsc::sync_channel(1);
        let writer = std::thread::spawn(move || {
            let result = input.write_all(&frame).and_then(|()| input.flush());
            let _ = sender.send((input, result.is_ok()));
        });
        let start = Instant::now();
        let result = loop {
            if let Err(error) = self.live(start, timeout, check) { break Err(error); }
            match receiver.try_recv() {
                Ok((input, ok)) => { self.input = Some(input); break if ok { Ok(()) } else { Err(WorkerFailure::Transport) }; }
                Err(mpsc::TryRecvError::Disconnected) => break Err(WorkerFailure::Transport),
                Err(mpsc::TryRecvError::Empty) => {}
            }
            std::thread::sleep(Duration::from_millis(5));
        };
        if result.is_err() {
            self.unusable = true;
            drop(self.guard.job.take()); self.guard.child.kill(); cancel_thread_io(&writer);
        }
        let cleanup = Instant::now();
        while !writer.is_finished() && cleanup.elapsed() < Duration::from_secs(2) {
            std::thread::sleep(Duration::from_millis(5));
        }
        if !writer.is_finished() { self.guard.poisoned.store(true, Ordering::Release); return Err(WorkerFailure::Cleanup); }
        writer.join().map_err(|_| WorkerFailure::Cleanup)?;
        result
    }
    fn receive<T: serde::de::DeserializeOwned>(&mut self, timeout: Duration, check: &impl Fn() -> Result<(), WorkerFailure>) -> Result<T, WorkerFailure> {
        let start = Instant::now();
        loop {
            self.live(start, timeout, check)?;
            match poll_pipe(&mut self.output).map_err(|_| WorkerFailure::Transport)? {
                PipePoll::Data(bytes) => self.buffer.extend(bytes),
                PipePoll::Closed if self.buffer.is_empty() => return Err(WorkerFailure::Transport),
                _ => {}
            }
            if let PipePoll::Data(bytes) = poll_pipe(&mut self.errors).map_err(|_| WorkerFailure::Transport)? {
                self.error_bytes += bytes.len();
                if self.error_bytes > STDERR_LIMIT { return Err(WorkerFailure::Protocol); }
            }
            if self.buffer.len() > FRAME_LIMIT as usize + 40 { return Err(WorkerFailure::Protocol); }
            if self.buffer.len() >= 8 {
                if self.buffer[..4] != custodian_ipc::MAGIC { return Err(WorkerFailure::Protocol); }
                let length = u32::from_le_bytes(self.buffer[4..8].try_into().map_err(|_| WorkerFailure::Protocol)?) as usize;
                if length > FRAME_LIMIT as usize || self.buffer.len() > length + 40 { return Err(WorkerFailure::Protocol); }
                if self.buffer.len() == length + 40 {
                    let reply = read_frame_limited(&mut std::io::Cursor::new(&self.buffer), &self.key, FRAME_LIMIT)
                        .map_err(|_| WorkerFailure::Protocol)?;
                    self.buffer.clear(); return Ok(reply);
                }
            }
            std::thread::sleep(Duration::from_millis(5));
        }
    }
    pub(crate) fn call(&mut self, action: crate::excel_protocol::Action, timeout: Duration,
        check: impl Fn() -> Result<(), WorkerFailure>) -> Result<crate::excel_protocol::Data, WorkerFailure> {
        if self.unusable { return Err(WorkerFailure::Cleanup); }
        check()?;
        if self.sequence >= 128 || timeout.is_zero() || timeout > Duration::from_secs(120) {
            return Err(WorkerFailure::InvalidConfiguration);
        }
        self.sequence += 1;
        let closing = matches!(&action, crate::excel_protocol::Action::Close { .. });
        let command = crate::excel_protocol::Command { session: self.session, sequence: self.sequence, operation: action };
        let mut frame = vec![];
        write_frame(&mut frame, &command, &self.key).map_err(|_| WorkerFailure::Protocol)?;
        let started = Instant::now();
        let result = (|| {
            self.send(frame, timeout, &check)?;
            let remaining = timeout.checked_sub(started.elapsed()).ok_or(WorkerFailure::Timeout)?;
            let reply: crate::excel_protocol::Reply = self.receive(remaining, &check)?;
            if reply.session != self.session || reply.sequence != self.sequence {
                return Err(WorkerFailure::Protocol);
            }
            if matches!(reply.result, crate::excel_protocol::Data::Refused) { return Err(WorkerFailure::Unavailable); }
            if closing && self.guard.child.wait_for(Duration::from_secs(2)).map_err(|_| WorkerFailure::Cleanup)? != Some(0) {
                return Err(WorkerFailure::Cleanup);
            }
            Ok(reply.result)
        })();
        if result.is_err() { self.unusable = true; drop(self.guard.job.take()); self.guard.child.kill(); }
        result
    }
}
impl Drop for WorkerSession<'_> {
    fn drop(&mut self) { self.key.fill(0); }
}

#[cfg(test)]
mod persistent_tests {
    use super::*;
    #[test]
    #[ignore = "requires explicitly built non-production session fault peer"]
    fn persistent_worker_faults_stop_dispatch_and_prove_process_cleanup() {
        let fixture = std::path::PathBuf::from(std::env::var_os("CUSTODIAN_SESSION_FIXTURE").expect("Explicit native fault peer required"));
        let temp = tempfile::tempdir().unwrap();
        for mode in ["session-start-hang","session-hang","session-stop","session-partial","session-oversize",
            "session-bad-mac","session-wrong-sequence","session-wrong-session","session-refused","session-revoked"] {
            let root = temp.path().join(mode); std::fs::create_dir(&root).unwrap();
            let executable = root.join("fault-peer.exe"); std::fs::copy(&fixture,&executable).unwrap();
            let program = WorkerProgram::verify(&executable, blake3::hash(&std::fs::read(&executable).unwrap()).to_hex().as_str()).unwrap();
            let config = crate::excel_protocol::Config { session:uuid::Uuid::new_v4(), source:root.join("source.xlsx"),
                output:root.join("output.xlsx"), existing_pids:vec![] };
            let started = Instant::now();
            let result = program.start_excel(config, Duration::from_secs(10), || Ok(()));
            if mode == "session-start-hang" {
                assert!(matches!(result,Err(WorkerFailure::Timeout)));
                assert!(started.elapsed() < Duration::from_secs(15));
            } else {
                let (mut session,_) = result.unwrap(); let call_start = Instant::now();
                let result = session.call(crate::excel_protocol::Action::Metadata, Duration::from_millis(750), || {
                    if mode == "session-revoked" { Err(WorkerFailure::AuthorityChanged) }
                    else if mode == "session-stop" && call_start.elapsed() >= Duration::from_millis(300) { Err(WorkerFailure::Stopped) }
                    else { Ok(()) }
                });
                let expected = match mode {
                    "session-hang"|"session-partial" => WorkerFailure::Timeout,
                    "session-stop" => WorkerFailure::Stopped,
                    "session-revoked" => WorkerFailure::AuthorityChanged,
                    "session-refused" => WorkerFailure::Unavailable,
                    _ => WorkerFailure::Protocol,
                };
                assert!(matches!(result,Err(error) if error == expected), "wrong persistent fault class: {mode}");
                if mode == "session-revoked" { assert!(!root.join("command-read.txt").exists()); }
                else {
                    assert!(root.join("command-read.txt").exists(), "peer did not enter the measured operation");
                    assert!(matches!(session.call(crate::excel_protocol::Action::Metadata,Duration::from_secs(1),|| Ok(())),Err(WorkerFailure::Cleanup)));
                }
                drop(session);
                assert!(call_start.elapsed() < Duration::from_secs(6));
            }
            let records = std::fs::read_to_string(root.join("worker-pids.txt")).expect("Actual peer startup must be proved");
            for record in records.lines() {
                let values = record.split_whitespace().collect::<Vec<_>>();
                assert!(!custodian_winsec::apps::process_is_running(values[0].parse().unwrap(),values[1].parse().unwrap()).unwrap(), "peer survived cleanup: {mode}");
            }
            eprintln!("PERSISTENT WORKER FAULT VERIFIED: {mode}");
        }
    }
}
