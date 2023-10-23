mod receiver;
mod connection;
mod handler;
mod fileio;

pub use receiver::*;

/// Default address to listen on
const ADDR: &'static str = "0.0.0.0";
/// Temporary PID file name
const PID_FILE: &'static str = ".receiver.pid";
/// Default channel capacity
#[allow(dead_code)]
const CHANNEL_CAPACITY: usize = 10;
