//! Two read-only questions for the Windows shell: how much the Recycle Bin
//! holds, and whether the person is busy (a fullscreen game, a presentation,
//! a locked screen) so Cubby keeps unprompted messages to itself (§14).

/// What the Recycle Bin holds, across all drives.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RecycleBin {
    pub items: u64,
    pub bytes: u64,
}

/// What Windows says about interrupting the person right now.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Attention {
    /// Fine to show a message.
    Available,
    /// Fullscreen app, game or presentation, or Windows says "busy".
    Busy,
    /// Locked, screensaver, or nobody signed in at the screen.
    Away,
}

/// Maps a `QUERY_USER_NOTIFICATION_STATE` value.
fn attention_from(state: i32) -> Attention {
    match state {
        5 => Attention::Available,            // QUNS_ACCEPTS_NOTIFICATIONS
        1 => Attention::Away,                 // QUNS_NOT_PRESENT
        2 | 3 | 4 | 6 | 7 => Attention::Busy, // busy, fullscreen, presenting, quiet time, Store app
        _ => Attention::Available,
    }
}

#[cfg(windows)]
#[allow(unsafe_code)]
pub fn recycle_bin() -> Option<RecycleBin> {
    use windows_sys::Win32::UI::Shell::{SHQueryRecycleBinW, SHQUERYRBINFO};
    let mut info = SHQUERYRBINFO {
        cbSize: std::mem::size_of::<SHQUERYRBINFO>() as u32,
        i64Size: 0,
        i64NumItems: 0,
    };
    // SAFETY: a null root asks about every drive; `info` is a valid,
    // writable struct with its size set as the API requires.
    let result = unsafe { SHQueryRecycleBinW(std::ptr::null(), &mut info) };
    (result == 0).then(|| RecycleBin {
        items: info.i64NumItems.max(0) as u64,
        bytes: info.i64Size.max(0) as u64,
    })
}

#[cfg(not(windows))]
pub fn recycle_bin() -> Option<RecycleBin> {
    None
}

#[cfg(windows)]
#[allow(unsafe_code)]
pub fn attention() -> Attention {
    use windows_sys::Win32::UI::Shell::SHQueryUserNotificationState;
    let mut state = 0i32;
    // SAFETY: `state` is a valid, writable i32 for the call.
    let result = unsafe { SHQueryUserNotificationState(&mut state) };
    if result == 0 {
        attention_from(state)
    } else {
        Attention::Available
    }
}

#[cfg(not(windows))]
pub fn attention() -> Attention {
    Attention::Available
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_the_notification_states() {
        assert_eq!(attention_from(5), Attention::Available);
        assert_eq!(attention_from(1), Attention::Away);
        for busy in [2, 3, 4, 6, 7] {
            assert_eq!(attention_from(busy), Attention::Busy, "{busy}");
        }
    }

    #[test]
    fn asking_the_real_shell_never_panics() {
        let _ = attention();
        if let Some(bin) = recycle_bin() {
            assert!(bin.items > 0 || bin.bytes == 0);
        }
    }
}
