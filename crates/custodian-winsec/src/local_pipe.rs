//! Private, local-only, single-client IPC. No listener on a network interface.
#[cfg(windows)]
pub use native::Server;

#[cfg(windows)]
#[allow(unsafe_code)]
mod native {
    use std::{
        fs::File,
        io,
        os::windows::io::{AsRawHandle, FromRawHandle},
        ptr,
    };
    use windows_sys::Win32::{
        Foundation::{CloseHandle, LocalFree, HANDLE, INVALID_HANDLE_VALUE},
        Security::Authorization::{
            ConvertSidToStringSidW, ConvertStringSecurityDescriptorToSecurityDescriptorW,
        },
        Security::{GetTokenInformation, TokenUser, SECURITY_ATTRIBUTES, TOKEN_QUERY, TOKEN_USER},
        Storage::FileSystem::{FILE_FLAG_FIRST_PIPE_INSTANCE, PIPE_ACCESS_DUPLEX},
        System::Pipes::{
            ConnectNamedPipe, CreateNamedPipeW, GetNamedPipeClientProcessId, PIPE_NOWAIT,
            PIPE_REJECT_REMOTE_CLIENTS, PIPE_TYPE_BYTE,
        },
        System::Threading::{GetCurrentProcess, OpenProcessToken},
    };

    fn denied() -> io::Error {
        io::Error::other("Private local pipe unavailable")
    }
    struct Allocation(*mut core::ffi::c_void);
    impl Drop for Allocation {
        fn drop(&mut self) {
            // SAFETY: both security conversion APIs allocate with LocalAlloc.
            unsafe {
                LocalFree(self.0);
            }
        }
    }

    fn security() -> io::Result<Allocation> {
        let mut token: HANDLE = ptr::null_mut();
        // SAFETY: valid pseudo process handle, writable output, query-only rights.
        if unsafe { OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) } == 0 {
            return Err(denied());
        }
        let result = (|| {
            let mut size = 0;
            // SAFETY: size query only; no output buffer is supplied.
            unsafe {
                GetTokenInformation(token, TokenUser, ptr::null_mut(), 0, &mut size);
            }
            if size == 0 || size > 65536 {
                return Err(denied());
            }
            let mut buffer = vec![0usize; (size as usize).div_ceil(std::mem::size_of::<usize>())];
            // SAFETY: aligned storage sized by the previous query, live token.
            if unsafe {
                GetTokenInformation(
                    token,
                    TokenUser,
                    buffer.as_mut_ptr().cast(),
                    size,
                    &mut size,
                )
            } == 0
            {
                return Err(denied());
            }
            // SAFETY: GetTokenInformation initialized a TOKEN_USER in aligned storage.
            let user = unsafe { &*buffer.as_ptr().cast::<TOKEN_USER>() };
            let mut sid = ptr::null_mut();
            // SAFETY: SID points inside the live token information buffer.
            if unsafe { ConvertSidToStringSidW(user.User.Sid, &mut sid) } == 0 {
                return Err(denied());
            }
            let _sid = Allocation(sid.cast());
            let mut length = 0;
            // SAFETY: the conversion produced a nul-terminated SID string.
            while unsafe { *sid.add(length) } != 0 {
                length += 1;
                if length > 256 {
                    return Err(denied());
                }
            }
            // SAFETY: the bounded string has exactly `length` initialized code units.
            let text = String::from_utf16(unsafe { std::slice::from_raw_parts(sid, length) })
                .map_err(|_| denied())?;
            let sddl = format!("D:P(A;;GA;;;SY)(A;;GA;;;{text})")
                .encode_utf16()
                .chain(Some(0))
                .collect::<Vec<_>>();
            let mut descriptor = ptr::null_mut();
            // SAFETY: valid SDDL with only system/current-user grants and a live output.
            if unsafe {
                ConvertStringSecurityDescriptorToSecurityDescriptorW(
                    sddl.as_ptr(),
                    1,
                    &mut descriptor,
                    ptr::null_mut(),
                )
            } == 0
            {
                return Err(denied());
            }
            Ok(Allocation(descriptor))
        })();
        // SAFETY: query token is owned here and closed once on every path.
        unsafe {
            CloseHandle(token);
        }
        result
    }

    pub struct Server {
        file: File,
        name: String,
        connected: bool,
    }
    impl Server {
        pub fn new(session: &str) -> io::Result<Self> {
            if session.len() != 36
                || !session.bytes().enumerate().all(|(i, b)| {
                    if [8, 13, 18, 23].contains(&i) {
                        b == b'-'
                    } else {
                        b.is_ascii_digit() || (b'a'..=b'f').contains(&b)
                    }
                })
            {
                return Err(denied());
            }
            let name = format!(r"\\.\pipe\custodian-editor-{session}");
            let encoded = name.encode_utf16().chain(Some(0)).collect::<Vec<_>>();
            let descriptor = security()?;
            let attributes = SECURITY_ATTRIBUTES {
                nLength: std::mem::size_of::<SECURITY_ATTRIBUTES>() as u32,
                lpSecurityDescriptor: descriptor.0,
                bInheritHandle: 0,
            };
            // SAFETY: live path/security descriptor. NOWAIT prevents host read/write
            // and connection calls from waiting on an unresponsive app. The DACL
            // admits this user/system only; remote clients are explicitly rejected.
            let handle = unsafe {
                CreateNamedPipeW(
                    encoded.as_ptr(),
                    PIPE_ACCESS_DUPLEX | FILE_FLAG_FIRST_PIPE_INSTANCE,
                    PIPE_TYPE_BYTE | PIPE_NOWAIT | PIPE_REJECT_REMOTE_CLIENTS,
                    1,
                    262184,
                    262184,
                    0,
                    &attributes,
                )
            };
            if handle == INVALID_HANDLE_VALUE {
                return Err(denied());
            }
            // SAFETY: transfer exactly this newly created owned pipe handle to File.
            let file = unsafe { File::from_raw_handle(handle) };
            Ok(Self {
                file,
                name,
                connected: false,
            })
        }
        pub fn name(&self) -> &str {
            &self.name
        }
        pub fn try_connect(&mut self) -> io::Result<bool> {
            if self.connected {
                return Ok(true);
            }
            // SAFETY: owned server pipe, NOWAIT mode, no overlapped storage.
            let ok = unsafe { ConnectNamedPipe(self.file.as_raw_handle(), ptr::null_mut()) };
            if ok != 0 {
                self.connected = true;
                return Ok(true);
            }
            match io::Error::last_os_error().raw_os_error() {
                Some(535) => {
                    self.connected = true;
                    Ok(true)
                } // ERROR_PIPE_CONNECTED
                Some(536) => Ok(false), // ERROR_PIPE_LISTENING
                _ => Err(denied()),
            }
        }
        pub fn client_pid(&self) -> io::Result<u32> {
            let mut pid = 0;
            // SAFETY: connected owned server pipe and valid output storage.
            if unsafe { GetNamedPipeClientProcessId(self.file.as_raw_handle(), &mut pid) } == 0
                || pid == 0
            {
                return Err(denied());
            }
            Ok(pid)
        }
        pub fn file(&mut self) -> &mut File {
            &mut self.file
        }
    }
}

#[cfg(all(test, windows))]
mod tests {
    use super::Server;
    use crate::worker_io::{poll_pipe, PipePoll};
    use std::{
        io::Write,
        process::{Command, Stdio},
        time::{Duration, Instant},
    };
    #[test]
    fn local_client_is_bound_and_bad_pipe_names_are_refused() {
        assert!(Server::new("../../arbitrary").is_err());
        let mut server = Server::new("18a142c7-c123-4122-873a-8f2707fc4063").unwrap();
        assert!(Server::new("18a142c7-c123-4122-873a-8f2707fc4063").is_err());
        let mut child = Command::new("node").args(["-e",
            "const n=require('net');const s=n.createConnection(process.argv[1]);s.on('connect',()=>s.write('hello'));s.on('data',d=>{if(d.toString()!=='reply')process.exit(2);s.end()});s.on('error',()=>process.exit(3));", server.name()])
            .stdout(Stdio::null()).stderr(Stdio::null()).spawn().unwrap();
        let start = Instant::now();
        let result = || {
            while !server.try_connect().unwrap() {
                assert!(start.elapsed() < Duration::from_secs(10));
                std::thread::sleep(Duration::from_millis(10));
            }
            assert_eq!(server.client_pid().unwrap(), child.id());
            loop {
                if let PipePoll::Data(bytes) = poll_pipe(server.file()).unwrap() {
                    assert_eq!(bytes, b"hello");
                    break;
                }
                assert!(start.elapsed() < Duration::from_secs(10));
                std::thread::sleep(Duration::from_millis(10));
            }
            server.file().write_all(b"reply").unwrap();
            while child.try_wait().unwrap().is_none() {
                assert!(start.elapsed() < Duration::from_secs(10));
                std::thread::sleep(Duration::from_millis(10));
            }
            assert!(child.try_wait().unwrap().unwrap().success());
        };
        let caught = std::panic::catch_unwind(std::panic::AssertUnwindSafe(result));
        let _ = child.kill();
        assert!(caught.is_ok(), "local IPC failed");
    }
}
