//! Audited cast helpers for the Win32 FFI boundary.
//!
//! Workspace lints deny `clippy::as_conversions`. Most conversions have an
//! exact, infallible std alternative (`i32::from`, `u32::try_from`,
//! saturating arithmetic) and must use it — these helpers cover only the
//! remainder, where no such alternative exists:
//!
//! * raw-pointer to integer round trips (`HWND` / `LPARAM` payloads) —
//!   bit-preserving reinterpretations between pointer-sized values;
//! * integer to `f32` for screen geometry — magnitudes are bounded by display
//!   coordinates, far below the 2^24 exact-conversion limit;
//! * `f32` to `u32` for icon sizes — inputs are small positive layout values,
//!   and float-`as` saturation semantics are exactly what is wanted.
//!
//! Each helper carries a targeted `allow` so every other `as` in the
//! codebase still fails the build.

use windows::Win32::Foundation::{HWND, LPARAM, WPARAM};

/// `HWND.0` to `isize` for storage in window lists and snapshots.
///
/// Bit-preserving reinterpretation of a pointer-sized handle; the exact
/// inverse of [`raw_to_hwnd`].
#[allow(clippy::as_conversions)] // documented pointer-sized round trip
pub fn hwnd_to_raw(hwnd: HWND) -> isize {
    hwnd.0 as isize
}

/// `isize` back to `HWND`. Only ever fed values from [`hwnd_to_raw`].
#[allow(clippy::as_conversions)] // inverse of `hwnd_to_raw`; never truncates
pub fn raw_to_hwnd(raw: isize) -> HWND {
    HWND(raw as *mut core::ffi::c_void)
}

/// `*mut T` to `LPARAM` for `EnumWindows` / hook payloads.
#[allow(clippy::as_conversions)] // pointer-sized; exact on all Windows targets
pub fn ptr_to_lparam<T>(ptr: *mut T) -> LPARAM {
    LPARAM(ptr as isize)
}

/// `LPARAM` back to `*mut T`.
///
/// # Safety
///
/// `lparam` must be a value produced by [`ptr_to_lparam`] for a live `T`.
#[allow(clippy::as_conversions)] // inverse of `ptr_to_lparam`; exact round trip
pub unsafe fn lparam_to_mut_ptr<T>(lparam: LPARAM) -> *mut T {
    lparam.0 as *mut T
}

/// `size_of::<T>()` as `u32` for Win32 `cbSize` fields.
///
/// Struct sizes cannot realistically exceed `u32::MAX`; saturation is a
/// defensive fallback, not an expected path.
pub fn size_of_u32<T>() -> u32 {
    u32::try_from(std::mem::size_of::<T>()).unwrap_or(u32::MAX)
}

/// `i32` pixels to `f32` logical units.
///
/// Exact for `|px| < 2^24`; display coordinates never approach that limit.
#[allow(clippy::as_conversions)] // no exact std conversion int -> float exists
pub fn px_to_f32(px: i32) -> f32 {
    px as f32
}

/// `u32` extent to `f32` logical units. See [`px_to_f32`].
#[allow(clippy::as_conversions)] // no exact std conversion int -> float exists
pub fn u32_to_f32(value: u32) -> f32 {
    value as f32
}

/// `usize` count to `f32` for list widths and icon math.
///
/// List lengths and icon sizes are tiny; precision loss is impossible here.
#[allow(clippy::as_conversions)] // no exact std conversion int -> float exists
pub fn usize_to_f32(value: usize) -> f32 {
    value as f32
}

/// `f32` layout size to `u32` pixels.
///
/// Inputs are small positive constants; float-`as` saturation on overflow or
/// `NaN` is the desired behavior.
#[allow(clippy::as_conversions)] // no saturating std conversion float -> int exists
pub fn f32_to_u32(value: f32) -> u32 {
    value as u32
}

/// `u16` virtual-key code to `i32` for `GetAsyncKeyState` / `keybd_event`.
pub fn vk_to_i32(vk: u16) -> i32 {
    i32::from(vk)
}

/// `isize` (`LONG_PTR` from `GetWindowLongPtrW`) to `u32` style flags.
///
/// Keeps the low 32 bits where window styles live — the standard
/// `GetWindowLong*` truncation, never an accidental loss.
#[allow(clippy::as_conversions)] // documented low-word truncation
pub fn long_ptr_to_u32(value: isize) -> u32 {
    value as u32
}

/// `u32` style flags back to `isize` for `SetWindowLongPtrW`.
/// Inverse of [`long_ptr_to_u32`].
#[allow(clippy::as_conversions)] // inverse of `long_ptr_to_u32`; exact on Windows
pub fn u32_to_long_ptr(value: u32) -> isize {
    value as isize
}

/// `isize` raw window value to `*mut c_void` for handle construction.
///
/// Same bit-preserving reinterpretation as [`raw_to_hwnd`], for call sites
/// that need the raw pointer rather than the `HWND` wrapper.
#[allow(clippy::as_conversions)] // pointer-sized; exact on all Windows targets
pub fn raw_to_mut_c_void(raw: isize) -> *mut core::ffi::c_void {
    raw as *mut core::ffi::c_void
}

/// `f64` physical pixels to `f32` logical units.
///
/// Exact for magnitudes below 2^24; tray and cursor coordinates qualify.
#[allow(clippy::as_conversions)] // no exact std conversion float -> float exists
pub fn f64_to_f32(value: f64) -> f32 {
    value as f32
}

/// `f64` physical pixels to `i32` for Win32 `POINT` construction.
///
/// Float-`as` saturation keeps extreme values in range instead of wrapping;
/// inputs are real screen coordinates.
#[allow(clippy::as_conversions)] // no saturating std conversion float -> int exists
pub fn f64_to_i32(value: f64) -> i32 {
    value as i32
}

/// `WPARAM` window-message id to `u32`.
///
/// Message ids are 32-bit (`UINT`) by definition; this keeps the low word
/// exactly as the message loop delivers it.
#[allow(clippy::as_conversions)] // documented low-word message id
pub fn wparam_to_u32(param: WPARAM) -> u32 {
    param.0 as u32
}

/// `i32` screen coordinate to `usize` for `WPARAM` packing.
///
/// Bit-preserving; the inverse [`usize_bits_to_i32`] keeps the low 32 bits,
/// so negative multi-monitor coordinates round-trip exactly.
#[allow(clippy::as_conversions)] // intentional bit-preserving pack
pub fn i32_to_usize_bits(value: i32) -> usize {
    value as usize
}

/// `usize` (`WPARAM`) payload back to `i32` screen coordinate.
/// Inverse of [`i32_to_usize_bits`].
#[allow(clippy::as_conversions)] // intentional bit-preserving unpack
pub fn usize_bits_to_i32(value: usize) -> i32 {
    value as i32
}

/// `i32` screen coordinate to `isize` for `LPARAM` packing.
///
/// Bit-preserving; the inverse [`isize_bits_to_i32`] keeps the low 32 bits,
/// so negative multi-monitor coordinates round-trip exactly.
#[allow(clippy::as_conversions)] // intentional bit-preserving pack
pub fn i32_to_isize_bits(value: i32) -> isize {
    value as isize
}

/// `isize` (`LPARAM`) payload back to `i32` screen coordinate.
///
/// Inverse of the exact `isize::from` widening used when packing; keeps the
/// low 32 bits so negative coordinates round-trip exactly.
#[allow(clippy::as_conversions)] // intentional bit-preserving unpack
pub fn isize_bits_to_i32(value: isize) -> i32 {
    value as i32
}

/// `LPARAM` to `*const T` for read-only hook payloads (`MSLLHOOKSTRUCT`,
/// `KBDLLHOOKSTRUCT`).
///
/// # Safety
///
/// `lparam` must be the hook-provided pointer to a live `T`, valid for the
/// duration of the hook callback.
#[allow(clippy::as_conversions)] // hook ABI hands us the pointer as an integer
pub unsafe fn lparam_to_const_ptr<T>(lparam: LPARAM) -> *const T {
    lparam.0 as *const T
}
