pub const MAX_CONCURRENT_REQUESTS: usize = 10;
pub const DEFAULT_CHUNK_SIZE: usize = 1024 * 1024 * 32; // 32 MB (standard S3 chunk)
pub const DEFAULT_CONCURRENT_MULTIPLIER: u8 = 10;
pub const MAX_VELOCITY: u8 = 10;
pub const MIN_VELOCITY: u8 = 1;
pub const CLIENT_ID: &str = env!("DCCMD_CLIENT_ID");
pub const CLIENT_SECRET: &str = env!("DCCMD_CLIENT_SECRET");
pub const APPLICATION_NAME: &str = "dccmd";
