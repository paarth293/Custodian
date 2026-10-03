use std::time::{SystemTime, UNIX_EPOCH};

static mut LAST_REAL_CLICK: u64 = 0;

pub fn record_real_click() {
    unsafe {
        LAST_REAL_CLICK = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_millis() as u64;
    }
}

pub fn verify_real_click() -> bool {
    let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_millis() as u64;
    unsafe {
        now - LAST_REAL_CLICK < 500
    }
}
