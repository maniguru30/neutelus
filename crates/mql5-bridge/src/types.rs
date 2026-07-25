use std::ffi::CStr;
use std::os::raw::c_char;

#[repr(C)]
pub enum Signal {
    None = 0,
    Buy = 1,
    Sell = 2,
    Exit = 3,
    BuyStrong = 4,
    SellStrong = 5,
    Error = -1,
}

/// SAFETY: `ptr` must be a valid null-terminated C string for the duration of the call.
/// The returned reference borrows from the memory pointed to by `ptr`.
pub unsafe fn cstr_to_str(ptr: *const c_char) -> &'static str {
    if ptr.is_null() {
        return "";
    }
    CStr::from_ptr(ptr).to_str().unwrap_or("")
}
