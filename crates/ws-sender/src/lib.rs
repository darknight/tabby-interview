mod sender;
pub use sender::*;

/// Default channel capacity
const CHANNEL_CAPACITY: usize = 10;
/// Default file chunk size
const FILE_CHUNK_SIZE: usize = 1024 * 1024 * 1;
