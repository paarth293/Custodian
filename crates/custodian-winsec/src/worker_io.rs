//! Nonblocking polling of private anonymous pipes and bounded cleanup support.
use std::fs::File;
use std::io;

pub enum PipePoll {
    Pending,
    Data(Vec<u8>),
    Closed,
}

#[cfg(windows)]
#[allow(unsafe_code)]
pub fn poll_pipe(file: &mut File) -> io::Result<PipePoll> {
    use std::io::Read;
    use std::os::windows::io::AsRawHandle;
    use windows_sys::Win32::System::Pipes::PeekNamedPipe;
    let mut available = 0;
    // SAFETY: the owned anonymous pipe is valid; only this caller reads it.
    let ok = unsafe {
        PeekNamedPipe(
            file.as_raw_handle(),
            std::ptr::null_mut(),
            0,
            std::ptr::null_mut(),
            &mut available,
            std::ptr::null_mut(),
        )
    };
    if ok == 0 {
        let error = io::Error::last_os_error();
        return if error.raw_os_error() == Some(109) {
            Ok(PipePoll::Closed)
        } else {
            Err(error)
        };
    }
    if available == 0 {
        return Ok(PipePoll::Pending);
    }
    let mut data = vec![0; available.min(4096) as usize];
    let n = file.read(&mut data)?;
    data.truncate(n);
    Ok(if n == 0 {
        PipePoll::Closed
    } else {
        PipePoll::Data(data)
    })
}

#[cfg(windows)]
#[allow(unsafe_code)]
pub fn cancel_thread_io(thread: &std::thread::JoinHandle<()>) {
    use std::os::windows::io::AsRawHandle;
    use windows_sys::Win32::System::IO::CancelSynchronousIo;
    // SAFETY: the native thread handle remains owned by the borrowed JoinHandle.
    // Cancellation is a request, not proof of completion; caller still checks it.
    unsafe { CancelSynchronousIo(thread.as_raw_handle()) };
}

#[cfg(not(windows))]
pub fn poll_pipe(_: &mut File) -> io::Result<PipePoll> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "Windows anonymous pipes required",
    ))
}

#[cfg(not(windows))]
pub fn cancel_thread_io(_: &std::thread::JoinHandle<()>) {}
