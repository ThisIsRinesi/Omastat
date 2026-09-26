use chrono::{DateTime, Local, Utc};

pub fn utc_now() -> DateTime<Utc> {
    Utc::now()
}

pub fn local_now() -> DateTime<Local> {
    Local::now()
}

pub fn unix_now() -> i64 {
    utc_now().timestamp()
}
