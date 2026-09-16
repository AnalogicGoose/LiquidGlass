//! Stable numeric error codes for the C ABI. Per the architecture doc's error
//! model (§33): no Rust panic may unwind across the boundary, so every entry
//! point catches panics and maps them to `ErrorPanic` instead.

/// Negative on failure, `Ok` (0) on success — the common C convention, safe
/// to check with `result < 0`.
#[repr(i32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SGResult {
    Ok = 0,
    ErrorNullPointer = -1,
    ErrorInvalidStructSize = -2,
    ErrorPanic = -3,
    ErrorInvalidTexture = -4,
}
