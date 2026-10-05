//! Battery and power state.

/// The battery's state, when the PC has one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Battery {
    /// Charge, 0–100.
    pub percent: u8,
    /// Plugged into mains power.
    pub plugged_in: bool,
    /// Windows battery saver is on.
    pub saver_on: bool,
    /// Estimated time left on battery, when Windows knows it.
    pub minutes_left: Option<u32>,
}

/// Decodes `SYSTEM_POWER_STATUS` fields; `None` when there is no battery.
fn decode(ac_line: u8, flag: u8, percent: u8, status: u8, seconds_left: u32) -> Option<Battery> {
    const NO_BATTERY: u8 = 128;
    const UNKNOWN: u8 = 255;
    if flag & NO_BATTERY != 0 || flag == UNKNOWN || percent > 100 {
        return None;
    }
    Some(Battery {
        percent,
        plugged_in: ac_line == 1,
        saver_on: status & 1 != 0,
        minutes_left: (seconds_left != u32::MAX).then_some(seconds_left / 60),
    })
}

/// The battery state, or `None` on a PC without a battery.
#[cfg(windows)]
#[allow(unsafe_code)]
pub fn battery() -> Option<Battery> {
    use windows_sys::Win32::System::Power::{GetSystemPowerStatus, SYSTEM_POWER_STATUS};
    let mut s = SYSTEM_POWER_STATUS {
        ACLineStatus: 0,
        BatteryFlag: 0,
        BatteryLifePercent: 0,
        SystemStatusFlag: 0,
        BatteryLifeTime: 0,
        BatteryFullLifeTime: 0,
    };
    // SAFETY: `s` is a valid, writable SYSTEM_POWER_STATUS for the call.
    if unsafe { GetSystemPowerStatus(&mut s) } == 0 {
        return None;
    }
    decode(
        s.ACLineStatus,
        s.BatteryFlag,
        s.BatteryLifePercent,
        s.SystemStatusFlag,
        s.BatteryLifeTime,
    )
}

#[cfg(not(windows))]
pub fn battery() -> Option<Battery> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_power_status() {
        assert_eq!(
            decode(0, 0, 57, 1, 5400),
            Some(Battery {
                percent: 57,
                plugged_in: false,
                saver_on: true,
                minutes_left: Some(90),
            })
        );
        let charging = decode(1, 8, 80, 0, u32::MAX).unwrap();
        assert!(charging.plugged_in);
        assert_eq!(charging.minutes_left, None);
        assert_eq!(decode(1, 128, 255, 0, u32::MAX), None, "desktop PC");
        assert_eq!(decode(1, 255, 255, 0, u32::MAX), None, "unknown");
    }

    #[test]
    fn reading_the_real_battery_never_panics() {
        if let Some(b) = battery() {
            assert!(b.percent <= 100);
        }
    }
}
