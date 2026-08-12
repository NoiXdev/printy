pub mod scheduler;
pub mod worker;

/// Retry schedule from the spec: 5 s, 30 s, 120 s, three attempts total.
pub const MAX_ATTEMPTS: i64 = 3;
pub const BACKOFF_SECS: [i64; 3] = [5, 30, 120];

/// Backoff after `attempts_so_far` failed attempts, in milliseconds.
pub fn backoff_ms(attempts_so_far: i64) -> i64 {
    let idx = attempts_so_far.clamp(0, BACKOFF_SECS.len() as i64 - 1) as usize;
    BACKOFF_SECS[idx] * 1_000
}
