//! Exports for the browser build. The page loads the dataset into this module once, then renders
//! each view here instead of asking a server. static/app.js is the other side.
//!
//! Strings cross as bytes: JavaScript writes an argument into memory from `alloc`, the function
//! takes it back, and a result is left in a buffer that JavaScript reads with `out_ptr`/`out_len`.

use std::cell::RefCell;

use crate::data::Dataset;
use crate::query::Params;
use crate::views;

thread_local! {
    static DATA: RefCell<Option<Dataset>> = const { RefCell::new(None) };
    static OUT: RefCell<Vec<u8>> = const { RefCell::new(Vec::new()) };
}

/// Reserves `len` bytes for an argument.
#[unsafe(no_mangle)]
pub extern "C" fn alloc(len: usize) -> *mut u8 {
    Box::into_raw(vec![0u8; len].into_boxed_slice()).cast()
}

/// Takes back an argument reserved with `alloc`.
unsafe fn arg(ptr: *mut u8, len: usize) -> Box<[u8]> {
    unsafe { Box::from_raw(std::ptr::slice_from_raw_parts_mut(ptr, len)) }
}

fn set_out(s: String) {
    OUT.set(s.into_bytes());
}

#[unsafe(no_mangle)]
pub extern "C" fn out_ptr() -> *const u8 {
    OUT.with_borrow(|o| o.as_ptr())
}

#[unsafe(no_mangle)]
pub extern "C" fn out_len() -> usize {
    OUT.with_borrow(|o| o.len())
}

/// Loads jobs.json. On failure, returns false and leaves the error as the result.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn load(ptr: *mut u8, len: usize) -> bool {
    let bytes = unsafe { arg(ptr, len) };
    match Dataset::from_json(&bytes) {
        Ok(ds) => {
            DATA.set(Some(ds));
            true
        }
        Err(e) => {
            set_out(format!("{e:#}"));
            false
        }
    }
}

/// Runs `f` on the dataset with the view a query string (without the "?") asks for.
fn with_query(qs: &[u8], f: impl FnOnce(&Dataset, &Params) -> String) {
    // The server answers a malformed query with an error; here it falls back to the default view.
    let p = serde_urlencoded::from_bytes::<Params>(qs).unwrap_or_default().normalize();
    set_out(DATA.with_borrow(|ds| ds.as_ref().map(|ds| f(ds, &p))).unwrap_or_default());
}

/// The page's HTML for a query string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn page(ptr: *mut u8, len: usize) {
    with_query(&unsafe { arg(ptr, len) }, |ds, p| views::page(ds, p).into_string());
}

/// The CSV export for a query string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn csv(ptr: *mut u8, len: usize) {
    with_query(&unsafe { arg(ptr, len) }, views::csv);
}

/// The CSV export's file name.
#[unsafe(no_mangle)]
pub extern "C" fn csv_filename() {
    set_out(DATA.with_borrow(|ds| ds.as_ref().map(views::csv_filename)).unwrap_or_default());
}
