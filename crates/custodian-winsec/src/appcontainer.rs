//! Runs a program inside a Windows AppContainer, the sandbox Windows uses
//! for Store apps (SR-B2, §88 layer 1).
//!
//! The container is given no capabilities at all, so the program cannot
//! reach the network, the local network or the person's files. It can read
//! only folders explicitly granted to it with [`Sandbox::grant_read`] (and
//! what Windows lets every app read, such as the system folders), and write
//! only to its own private folder. It inherits exactly its three standard
//! pipes and no other handle, and starts suspended so a job's limits can be
//! put in place before it runs a single instruction.

use std::ffi::OsStr;
use std::fs::File;
use std::io;
use std::path::{Path, PathBuf};

/// Quotes one argument for a Windows command line, following the rules
/// programs use to split it again (`CommandLineToArgvW`).
fn quote(arg: &OsStr, out: &mut Vec<u16>) {
    use std::os::windows::ffi::OsStrExt;
    let units: Vec<u16> = arg.encode_wide().collect();
    let plain = !units.is_empty()
        && !units.iter().any(|&c| {
            c == b' ' as u16
                || c == b'\t' as u16
                || c == b'\n' as u16
                || c == 0x0b
                || c == b'"' as u16
        });
    if plain {
        out.extend(units);
        return;
    }
    out.push(b'"' as u16);
    let mut backslashes = 0usize;
    for &c in &units {
        if c == b'\\' as u16 {
            backslashes += 1;
            continue;
        }
        if c == b'"' as u16 {
            // Backslashes before a quote are doubled, and the quote escaped.
            out.extend(std::iter::repeat_n(b'\\' as u16, backslashes * 2 + 1));
        } else {
            out.extend(std::iter::repeat_n(b'\\' as u16, backslashes));
        }
        backslashes = 0;
        out.push(c);
    }
    // Backslashes before the closing quote are doubled.
    out.extend(std::iter::repeat_n(b'\\' as u16, backslashes * 2));
    out.push(b'"' as u16);
}

/// The command line for `program` and `args`, NUL-terminated.
fn command_line(program: &Path, args: &[&OsStr]) -> Vec<u16> {
    let mut line = Vec::new();
    // The program is always quoted, so a path with spaces stays one argument.
    {
        use std::os::windows::ffi::OsStrExt;
        line.push(b'"' as u16);
        line.extend(program.as_os_str().encode_wide());
        line.push(b'"' as u16);
    }
    for arg in args {
        line.push(b' ' as u16);
        quote(arg, &mut line);
    }
    line.push(0);
    line
}

/// A Unicode environment block holding exactly `vars`.
fn environment_block(vars: &[(&str, &OsStr)]) -> Vec<u16> {
    use std::os::windows::ffi::OsStrExt;
    let mut sorted: Vec<&(&str, &OsStr)> = vars.iter().collect();
    sorted.sort_by_key(|(k, _)| k.to_ascii_uppercase());
    let mut block = Vec::new();
    for (key, value) in sorted {
        block.extend(OsStr::new(key).encode_wide());
        block.push(b'=' as u16);
        block.extend(value.encode_wide());
        block.push(0);
    }
    block.push(0);
    if block.len() == 1 {
        block.push(0); // An empty block is two NULs.
    }
    block
}

pub use imp::{Contained, Sandbox};

#[allow(unsafe_code)]
mod imp {
    use super::{command_line, environment_block};
    use std::ffi::OsStr;
    use std::fs::File;
    use std::io;
    use std::os::windows::ffi::OsStrExt;
    use std::os::windows::io::FromRawHandle;
    use std::path::{Path, PathBuf};
    use std::ptr::{null, null_mut};
    use windows_sys::Win32::Foundation::{
        CloseHandle, LocalFree, SetHandleInformation, HANDLE, HANDLE_FLAG_INHERIT,
    };
    use windows_sys::Win32::Security::Authorization::{
        GetNamedSecurityInfoW, SetEntriesInAclW, SetNamedSecurityInfoW, EXPLICIT_ACCESS_W,
        GRANT_ACCESS, NO_MULTIPLE_TRUSTEE, SE_FILE_OBJECT, TRUSTEE_IS_SID,
        TRUSTEE_IS_WELL_KNOWN_GROUP, TRUSTEE_W,
    };
    use windows_sys::Win32::Security::Isolation::{
        CreateAppContainerProfile, DeleteAppContainerProfile,
        DeriveAppContainerSidFromAppContainerName,
    };
    use windows_sys::Win32::Security::{
        EqualSid, FreeSid, GetAce, ACCESS_ALLOWED_ACE, ACE_HEADER, ACL, DACL_SECURITY_INFORMATION,
        PSECURITY_DESCRIPTOR, PSID, SECURITY_ATTRIBUTES, SECURITY_CAPABILITIES,
        SUB_CONTAINERS_AND_OBJECTS_INHERIT,
    };
    use windows_sys::Win32::Storage::FileSystem::{FILE_GENERIC_EXECUTE, FILE_GENERIC_READ};
    use windows_sys::Win32::System::Pipes::CreatePipe;
    use windows_sys::Win32::System::Threading::UpdateProcThreadAttribute;
    use windows_sys::Win32::System::Threading::{
        CreateProcessW, DeleteProcThreadAttributeList, GetExitCodeProcess,
        InitializeProcThreadAttributeList, ResumeThread, TerminateProcess, CREATE_NO_WINDOW,
        CREATE_SUSPENDED, CREATE_UNICODE_ENVIRONMENT, EXTENDED_STARTUPINFO_PRESENT,
        LPPROC_THREAD_ATTRIBUTE_LIST, PROCESS_INFORMATION, PROC_THREAD_ATTRIBUTE_HANDLE_LIST,
        PROC_THREAD_ATTRIBUTE_SECURITY_CAPABILITIES, STARTF_USESTDHANDLES, STARTUPINFOEXW,
    };

    /// Variables Windows redirects to the container's folder at start-up.
    const PROFILE_VARIABLES: &[&str] = &[
        "SystemRoot",
        "windir",
        "USERPROFILE",
        "LOCALAPPDATA",
        "APPDATA",
        "TEMP",
        "TMP",
        "HOMEDRIVE",
        "HOMEPATH",
    ];

    /// `HRESULT_FROM_WIN32(ERROR_ALREADY_EXISTS)`: the profile exists.
    const ALREADY_EXISTS: i32 = 0x8007_00B7_u32 as i32;
    /// The exit code of a process still running.
    const STILL_ACTIVE: u32 = 259;
    /// `ACCESS_ALLOWED_ACE_TYPE`.
    const ACCESS_ALLOWED: u8 = 0;

    fn wide(text: &OsStr) -> Vec<u16> {
        text.encode_wide().chain(Some(0)).collect()
    }

    fn hresult(result: i32) -> io::Result<()> {
        if result >= 0 {
            Ok(())
        } else {
            Err(io::Error::other(format!("HRESULT 0x{:08X}", result as u32)))
        }
    }

    fn win32(error: u32) -> io::Result<()> {
        if error == 0 {
            Ok(())
        } else {
            Err(io::Error::from_raw_os_error(error as i32))
        }
    }

    fn check(ok: i32) -> io::Result<()> {
        if ok == 0 {
            Err(io::Error::last_os_error())
        } else {
            Ok(())
        }
    }

    /// Names the Win32 call behind an error, for diagnosing other PCs.
    fn step(name: &str, result: io::Result<()>) -> io::Result<()> {
        result.map_err(|e| io::Error::new(e.kind(), format!("{name}: {e}")))
    }

    /// Memory Windows allocated with LocalAlloc, freed when dropped.
    struct LocalMemory(*mut core::ffi::c_void);

    impl Drop for LocalMemory {
        fn drop(&mut self) {
            if !self.0.is_null() {
                // SAFETY: the pointer came from an API that documents
                // LocalFree as its release function, and is freed once.
                unsafe { LocalFree(self.0) };
            }
        }
    }

    /// An open handle, closed when dropped unless taken.
    struct Handle(HANDLE);

    impl Handle {
        fn take(mut self) -> HANDLE {
            std::mem::replace(&mut self.0, null_mut())
        }
    }

    impl Drop for Handle {
        fn drop(&mut self) {
            if !self.0.is_null() {
                // SAFETY: the handle is open and owned by this value.
                unsafe { CloseHandle(self.0) };
            }
        }
    }

    /// A pipe whose `child` end is inherited and whose other end stays here.
    fn pipe(child_reads: bool) -> io::Result<(Handle, Handle)> {
        let attributes = SECURITY_ATTRIBUTES {
            nLength: std::mem::size_of::<SECURITY_ATTRIBUTES>() as u32,
            lpSecurityDescriptor: null_mut(),
            bInheritHandle: 1,
        };
        let (mut read, mut write): (HANDLE, HANDLE) = (null_mut(), null_mut());
        // SAFETY: both outputs are valid places for handles and the
        // attributes outlive the call.
        check(unsafe { CreatePipe(&mut read, &mut write, &attributes, 0) })?;
        let (read, write) = (Handle(read), Handle(write));
        let (child, ours) = if child_reads {
            (read, write)
        } else {
            (write, read)
        };
        // SAFETY: `ours` is an open handle; this only clears its inherit flag.
        check(unsafe { SetHandleInformation(ours.0, HANDLE_FLAG_INHERIT, 0) })?;
        Ok((child, ours))
    }

    /// An AppContainer profile and its SID.
    pub struct Sandbox {
        name: String,
        sid: PSID,
    }

    // SAFETY: the SID is plain memory owned by this value and never mutated.
    unsafe impl Send for Sandbox {}
    // SAFETY: as above; shared use only reads the SID.
    unsafe impl Sync for Sandbox {}

    impl Sandbox {
        /// Opens the AppContainer `name`, creating it the first time.
        pub fn open(name: &str) -> io::Result<Self> {
            let name_w = wide(OsStr::new(name));
            let mut sid: PSID = null_mut();
            // SAFETY: the strings are NUL-terminated and outlive the call; no
            // capabilities are passed (null and 0); `sid` receives the SID.
            let created = unsafe {
                CreateAppContainerProfile(
                    name_w.as_ptr(),
                    name_w.as_ptr(),
                    name_w.as_ptr(),
                    null(),
                    0,
                    &mut sid,
                )
            };
            if created == ALREADY_EXISTS {
                // SAFETY: as above; `sid` receives the existing profile's SID.
                hresult(unsafe {
                    DeriveAppContainerSidFromAppContainerName(name_w.as_ptr(), &mut sid)
                })?;
            } else {
                hresult(created)?;
            }
            Ok(Self {
                name: name.to_string(),
                sid,
            })
        }

        /// Removes the AppContainer `name` and its private folder.
        pub fn delete(name: &str) -> io::Result<()> {
            let name_w = wide(OsStr::new(name));
            // SAFETY: the name is NUL-terminated and outlives the call.
            hresult(unsafe { DeleteAppContainerProfile(name_w.as_ptr()) })
        }

        /// The container's own folder, the only place it may write.
        pub fn private_dir(&self) -> Option<PathBuf> {
            let local = std::env::var_os("LOCALAPPDATA")?;
            let dir = Path::new(&local)
                .join("Packages")
                .join(&self.name)
                .join("AC");
            dir.is_dir().then_some(dir)
        }

        /// Whether the DACL already allows this container to read.
        fn already_granted(&self, dacl: *const ACL) -> bool {
            if dacl.is_null() {
                return true; // No DACL at all: everyone may read.
            }
            // SAFETY: `dacl` points into a security descriptor that the
            // caller keeps alive for this call.
            let count = unsafe { (*dacl).AceCount };
            for index in 0..u32::from(count) {
                let mut ace: *mut core::ffi::c_void = null_mut();
                // SAFETY: `index` is below the ACL's ACE count.
                if unsafe { GetAce(dacl, index, &mut ace) } == 0 || ace.is_null() {
                    continue;
                }
                // SAFETY: every ACE starts with an ACE_HEADER.
                let header = unsafe { &*(ace as *const ACE_HEADER) };
                if header.AceType != ACCESS_ALLOWED {
                    continue;
                }
                // SAFETY: an access-allowed ACE has this layout, with the
                // SID starting at `SidStart`.
                let allowed = unsafe { &*(ace as *const ACCESS_ALLOWED_ACE) };
                let ace_sid = &allowed.SidStart as *const u32 as PSID;
                let wanted = FILE_GENERIC_READ | FILE_GENERIC_EXECUTE;
                // SAFETY: both are valid SIDs for the duration of the call.
                if unsafe { EqualSid(ace_sid, self.sid) } != 0 && allowed.Mask & wanted == wanted {
                    return true;
                }
            }
            false
        }

        /// Lets the container read (and run) `path`, and for a folder
        /// everything in it. Returns whether anything changed. Nothing else
        /// in the folder's permissions is touched.
        pub fn grant_read(&self, path: &Path) -> io::Result<bool> {
            let path_w = wide(path.as_os_str());
            let mut dacl: *mut ACL = null_mut();
            let mut descriptor: PSECURITY_DESCRIPTOR = null_mut();
            // SAFETY: the path is NUL-terminated; only the DACL and the
            // descriptor that owns it are requested.
            win32(unsafe {
                GetNamedSecurityInfoW(
                    path_w.as_ptr(),
                    SE_FILE_OBJECT,
                    DACL_SECURITY_INFORMATION,
                    null_mut(),
                    null_mut(),
                    &mut dacl,
                    null_mut(),
                    &mut descriptor,
                )
            })?;
            let _descriptor = LocalMemory(descriptor);
            if self.already_granted(dacl) {
                return Ok(false);
            }
            let access = EXPLICIT_ACCESS_W {
                grfAccessPermissions: FILE_GENERIC_READ | FILE_GENERIC_EXECUTE,
                grfAccessMode: GRANT_ACCESS,
                grfInheritance: if path.is_dir() {
                    SUB_CONTAINERS_AND_OBJECTS_INHERIT
                } else {
                    0
                },
                Trustee: TRUSTEE_W {
                    pMultipleTrustee: null_mut(),
                    MultipleTrusteeOperation: NO_MULTIPLE_TRUSTEE,
                    TrusteeForm: TRUSTEE_IS_SID,
                    TrusteeType: TRUSTEE_IS_WELL_KNOWN_GROUP,
                    ptstrName: self.sid as *mut u16,
                },
            };
            let mut new_dacl: *mut ACL = null_mut();
            // SAFETY: one valid entry; the old DACL is alive (owned by
            // `_descriptor`); the new one is returned in `new_dacl`.
            win32(unsafe { SetEntriesInAclW(1, &access, dacl, &mut new_dacl) })?;
            let _new_dacl = LocalMemory(new_dacl.cast());
            // SAFETY: the path is NUL-terminated and the new DACL is valid.
            win32(unsafe {
                SetNamedSecurityInfoW(
                    path_w.as_ptr(),
                    SE_FILE_OBJECT,
                    DACL_SECURITY_INFORMATION,
                    null_mut(),
                    null_mut(),
                    new_dacl,
                    null(),
                )
            })?;
            Ok(true)
        }

        /// Starts `program` in the container, suspended. Only `env` is in its
        /// environment and only its three pipes are inherited. Call
        /// [`Contained::resume`] once it is in its job.
        pub fn spawn(
            &self,
            program: &Path,
            args: &[&OsStr],
            env: &[(&str, &OsStr)],
            cwd: Option<&Path>,
        ) -> io::Result<Contained> {
            let (child_in, our_in) = pipe(true)?;
            let (child_out, our_out) = pipe(false)?;
            let (child_err, our_err) = pipe(false)?;

            let mut capabilities = SECURITY_CAPABILITIES {
                AppContainerSid: self.sid,
                Capabilities: null_mut(),
                CapabilityCount: 0,
                Reserved: 0,
            };
            let inherited: [HANDLE; 3] = [child_in.0, child_out.0, child_err.0];

            let mut size = 0usize;
            // SAFETY: a null list asks for the size needed for 2 attributes.
            unsafe { InitializeProcThreadAttributeList(null_mut(), 2, 0, &mut size) };
            let mut buffer = vec![0usize; size.div_ceil(std::mem::size_of::<usize>()).max(1)];
            let list = buffer.as_mut_ptr() as LPPROC_THREAD_ATTRIBUTE_LIST;
            // SAFETY: `buffer` is at least `size` bytes and suitably aligned.
            let ok = unsafe { InitializeProcThreadAttributeList(list, 2, 0, &mut size) };
            step("InitializeProcThreadAttributeList", check(ok))?;
            struct List(LPPROC_THREAD_ATTRIBUTE_LIST);
            impl Drop for List {
                fn drop(&mut self) {
                    // SAFETY: the list was initialised and is deleted once.
                    unsafe { DeleteProcThreadAttributeList(self.0) };
                }
            }
            let _list = List(list);
            // SAFETY: `capabilities` outlives CreateProcessW below, and its
            // exact size is passed.
            let ok = unsafe {
                UpdateProcThreadAttribute(
                    list,
                    0,
                    PROC_THREAD_ATTRIBUTE_SECURITY_CAPABILITIES as usize,
                    (&mut capabilities as *mut SECURITY_CAPABILITIES).cast(),
                    std::mem::size_of::<SECURITY_CAPABILITIES>(),
                    null_mut(),
                    null(),
                )
            };
            step("UpdateProcThreadAttribute(capabilities)", check(ok))?;
            // SAFETY: `inherited` outlives CreateProcessW below, and its
            // exact size is passed.
            let ok = unsafe {
                UpdateProcThreadAttribute(
                    list,
                    0,
                    PROC_THREAD_ATTRIBUTE_HANDLE_LIST as usize,
                    inherited.as_ptr().cast(),
                    std::mem::size_of_val(&inherited),
                    null_mut(),
                    null(),
                )
            };
            step("UpdateProcThreadAttribute(handles)", check(ok))?;

            let mut startup = STARTUPINFOEXW::default();
            startup.StartupInfo.cb = std::mem::size_of::<STARTUPINFOEXW>() as u32;
            startup.StartupInfo.dwFlags = STARTF_USESTDHANDLES;
            startup.StartupInfo.hStdInput = child_in.0;
            startup.StartupInfo.hStdOutput = child_out.0;
            startup.StartupInfo.hStdError = child_err.0;
            startup.lpAttributeList = list;

            let mut line = command_line(program, args);
            // Windows points these at the container's own folder when it
            // starts the program, and refuses to start it without them.
            let profile: Vec<(&str, std::ffi::OsString)> = PROFILE_VARIABLES
                .iter()
                .filter(|name| !env.iter().any(|(k, _)| k.eq_ignore_ascii_case(name)))
                .filter_map(|name| Some((*name, std::env::var_os(name)?)))
                .collect();
            let mut all: Vec<(&str, &OsStr)> = env.to_vec();
            all.extend(profile.iter().map(|(k, v)| (*k, v.as_os_str())));
            let block = environment_block(&all);
            let cwd_w = cwd.map(|p| wide(p.as_os_str()));
            let mut info = PROCESS_INFORMATION {
                hProcess: null_mut(),
                hThread: null_mut(),
                dwProcessId: 0,
                dwThreadId: 0,
            };
            // SAFETY: every pointer is valid for the call: the mutable
            // command line, the environment block, the optional directory,
            // the startup information (whose attribute list holds pointers
            // to `capabilities` and `inherited`, both still alive), and the
            // output structure.
            let ok = unsafe {
                CreateProcessW(
                    null(),
                    line.as_mut_ptr(),
                    null(),
                    null(),
                    1,
                    EXTENDED_STARTUPINFO_PRESENT
                        | CREATE_SUSPENDED
                        | CREATE_NO_WINDOW
                        | CREATE_UNICODE_ENVIRONMENT,
                    block.as_ptr().cast(),
                    cwd_w.as_ref().map_or(null(), |c| c.as_ptr()),
                    &startup.StartupInfo,
                    &mut info,
                )
            };
            step("CreateProcessW", check(ok))?;
            // The child has its copies; close ours of its ends.
            drop((child_in, child_out, child_err));
            let process = Handle(info.hProcess);
            let thread = Handle(info.hThread);
            // SAFETY: each handle is open, owned here, and handed to exactly
            // one File, which closes it.
            let (stdin, stdout, stderr) = unsafe {
                (
                    File::from_raw_handle(our_in.take()),
                    File::from_raw_handle(our_out.take()),
                    File::from_raw_handle(our_err.take()),
                )
            };
            Ok(Contained {
                process: process.take() as isize,
                thread: thread.take() as isize,
                pid: info.dwProcessId,
                stdin: Some(stdin),
                stdout: Some(stdout),
                stderr: Some(stderr),
            })
        }
    }

    impl Drop for Sandbox {
        fn drop(&mut self) {
            if !self.sid.is_null() {
                // SAFETY: the SID came from CreateAppContainerProfile or
                // DeriveAppContainerSidFromAppContainerName, both freed with
                // FreeSid, exactly once.
                unsafe { FreeSid(self.sid) };
            }
        }
    }

    /// A program running in a container. Dropping it closes the handles but
    /// does not end the program; its job does that.
    pub struct Contained {
        process: isize,
        thread: isize,
        pub pid: u32,
        pub stdin: Option<File>,
        pub stdout: Option<File>,
        pub stderr: Option<File>,
    }

    impl Contained {
        /// The process handle, for putting it in a job.
        pub fn raw_process(&self) -> isize {
            self.process
        }

        /// Lets the suspended program start.
        pub fn resume(&self) -> io::Result<()> {
            // SAFETY: the thread handle is open while `self` lives.
            if unsafe { ResumeThread(self.thread as HANDLE) } == u32::MAX {
                return Err(io::Error::last_os_error());
            }
            Ok(())
        }

        /// The exit code, or `None` while it runs.
        pub fn try_wait(&self) -> io::Result<Option<u32>> {
            let mut code = 0u32;
            // SAFETY: the process handle is open while `self` lives.
            check(unsafe { GetExitCodeProcess(self.process as HANDLE, &mut code) })?;
            Ok((code != STILL_ACTIVE).then_some(code))
        }

        /// Waits up to `limit` for the program to end; its exit code if it did.
        pub fn wait_for(&self, limit: std::time::Duration) -> io::Result<Option<u32>> {
            let start = std::time::Instant::now();
            loop {
                if let Some(code) = self.try_wait()? {
                    return Ok(Some(code));
                }
                if start.elapsed() >= limit {
                    return Ok(None);
                }
                std::thread::sleep(std::time::Duration::from_millis(20));
            }
        }

        /// Ends the program.
        pub fn kill(&self) {
            // SAFETY: the process handle is open while `self` lives.
            unsafe { TerminateProcess(self.process as HANDLE, 1) };
        }
    }

    impl Drop for Contained {
        fn drop(&mut self) {
            // SAFETY: both handles are open and owned by this value.
            unsafe {
                CloseHandle(self.thread as HANDLE);
                CloseHandle(self.process as HANDLE);
            }
        }
    }
}

/// A one-line summary of a container's stdout, for tests and logs.
pub fn read_all(file: &mut File) -> io::Result<String> {
    use std::io::Read;
    let mut text = String::new();
    file.read_to_string(&mut text)?;
    Ok(text)
}

/// The folders a sandbox needs read access to, without repeats or folders
/// already inside another one in the list.
pub fn distinct_roots(paths: &[PathBuf]) -> Vec<PathBuf> {
    let mut roots: Vec<PathBuf> = Vec::new();
    let mut sorted: Vec<&PathBuf> = paths.iter().collect();
    sorted.sort_by_key(|p| p.components().count());
    for path in sorted {
        let lower = |p: &Path| p.to_string_lossy().to_lowercase();
        if roots.iter().any(|r| {
            let (r, p) = (lower(r), lower(path));
            p == r || p.starts_with(&format!("{}\\", r.trim_end_matches('\\')))
        }) {
            continue;
        }
        roots.push(path.clone());
    }
    roots
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    fn line(program: &str, args: &[&str]) -> String {
        let args: Vec<&OsStr> = args.iter().map(OsStr::new).collect();
        let mut wide = command_line(Path::new(program), &args);
        wide.pop();
        String::from_utf16(&wide).unwrap()
    }

    #[test]
    fn quotes_arguments_the_way_programs_split_them() {
        assert_eq!(
            line("C:\\a b\\node.exe", &["x.js"]),
            "\"C:\\a b\\node.exe\" x.js"
        );
        assert_eq!(line("p", &["has space"]), "\"p\" \"has space\"");
        assert_eq!(line("p", &["say \"hi\""]), "\"p\" \"say \\\"hi\\\"\"");
        assert_eq!(line("p", &["C:\\dir\\"]), "\"p\" C:\\dir\\");
        assert_eq!(line("p", &["C:\\my dir\\"]), "\"p\" \"C:\\my dir\\\\\"");
        assert_eq!(line("p", &[""]), "\"p\" \"\"");
    }

    #[test]
    fn builds_an_environment_with_only_what_is_given() {
        let block = environment_block(&[("B", OsStr::new("2")), ("a", OsStr::new("1"))]);
        let text = String::from_utf16(&block).unwrap();
        assert_eq!(text, "a=1\0B=2\0\0");
        assert_eq!(environment_block(&[]), vec![0, 0]);
    }

    #[test]
    fn keeps_only_top_folders() {
        let roots = distinct_roots(&[
            PathBuf::from("C:\\repo\\node_modules\\x"),
            PathBuf::from("C:\\repo\\node_modules"),
            PathBuf::from("C:\\repo\\models"),
            PathBuf::from("D:\\node"),
            PathBuf::from("c:\\REPO\\models"),
        ]);
        assert_eq!(
            roots,
            vec![
                PathBuf::from("D:\\node"),
                PathBuf::from("C:\\repo\\node_modules"),
                PathBuf::from("C:\\repo\\models"),
            ]
        );
    }

    /// A unique container per test, removed afterwards.
    struct TestSandbox {
        name: String,
        sandbox: Option<Sandbox>,
    }

    impl TestSandbox {
        /// Each test gets its own container, since tests run in parallel.
        fn new(test: &str) -> Self {
            let name = format!("custodian.test.{}.{test}", std::process::id());
            let sandbox = Sandbox::open(&name).unwrap();
            Self {
                name,
                sandbox: Some(sandbox),
            }
        }

        fn run(&self, program: &str, args: &[&str]) -> (Option<u32>, String) {
            let system_root = std::env::var_os("SystemRoot").unwrap();
            let system32 = Path::new(&system_root).join("System32");
            let args: Vec<&OsStr> = args.iter().map(OsStr::new).collect();
            let mut child = self
                .sandbox
                .as_ref()
                .unwrap()
                .spawn(
                    &system32.join(program),
                    &args,
                    &[("SystemRoot", system_root.as_os_str())],
                    Some(&system32),
                )
                .unwrap();
            drop(child.stdin.take());
            child.resume().unwrap();
            let code = child.wait_for(Duration::from_secs(20)).unwrap();
            let mut out = child.stdout.take().unwrap();
            let text = read_all(&mut out).unwrap_or_default();
            (code, text)
        }
    }

    impl Drop for TestSandbox {
        fn drop(&mut self) {
            drop(self.sandbox.take());
            let _ = Sandbox::delete(&self.name);
        }
    }

    #[test]
    fn runs_a_program_inside_with_its_own_pipes() {
        let sandbox = TestSandbox::new("pipes");
        let (code, out) = sandbox.run("cmd.exe", &["/c", "echo contained"]);
        assert_eq!(code, Some(0));
        assert_eq!(out.trim(), "contained");
    }

    #[test]
    fn cannot_read_the_persons_files_until_granted_and_never_write() {
        let sandbox = TestSandbox::new("files");
        let dir = tempfile::tempdir().unwrap();
        let secret = dir.path().join("secret.txt");
        std::fs::write(&secret, "private").unwrap();
        let path = secret.display().to_string();

        let (_, out) = sandbox.run("cmd.exe", &["/c", "type", &path]);
        assert!(!out.contains("private"), "read without a grant: {out}");

        assert!(sandbox
            .sandbox
            .as_ref()
            .unwrap()
            .grant_read(dir.path())
            .unwrap());
        assert!(
            !sandbox
                .sandbox
                .as_ref()
                .unwrap()
                .grant_read(dir.path())
                .unwrap(),
            "granted once"
        );
        let (code, out) = sandbox.run("cmd.exe", &["/c", "type", &path]);
        assert_eq!(code, Some(0));
        assert_eq!(out.trim(), "private");

        let target = dir.path().join("written.txt").display().to_string();
        sandbox.run("cmd.exe", &["/c", &format!("echo x> \"{target}\"")]);
        assert!(!dir.path().join("written.txt").exists(), "read-only grant");
    }

    #[test]
    fn cannot_reach_the_network() {
        let sandbox = TestSandbox::new("network");
        let (code, out) = sandbox.run(
            "curl.exe",
            &[
                "-s",
                "-m",
                "8",
                "-o",
                "NUL",
                "-w",
                "%{http_code}",
                "http://example.com/",
            ],
        );
        assert_ne!(code, Some(0), "curl reached the network: {out}");
        assert!(!out.starts_with('2') && !out.starts_with('3'), "{out}");
    }
}
