//! The X11 calls that stand in for the Win32 ones on Linux: the pointer's button state (carrying the
//! notch), the active window (seen-clears-it), the window list and raising a window (jump-back).
//! libX11 is loaded with dlopen, so a session without an X server simply gets "nothing found".

use std::ffi::CString;
use std::os::raw::{c_int, c_long, c_uchar, c_uint, c_ulong};
use x11_dl::xlib::{self, Display, Xlib};

struct Conn {
    x: Xlib,
    d: *mut Display,
}

// One connection per thread: Xlib is not thread-safe without XInitThreads, and the carry loop
// asks for the button state every frame, so opening a display each time would be wasteful.
thread_local! {
    static CONN: Option<Conn> = open();
}

/// Must run before any other Xlib call in the process, GTK's included: the pointer, focus and
/// carry threads each hold their own connection, and Xlib's shared state needs its locks for that.
pub fn init_threads() {
    if let Ok(x) = Xlib::open() {
        unsafe { (x.XInitThreads)() };
    }
}

fn open() -> Option<Conn> {
    let x = Xlib::open().ok()?;
    let d = unsafe { (x.XOpenDisplay)(std::ptr::null()) };
    (!d.is_null()).then_some(Conn { x, d })
}

fn with<T>(f: impl FnOnce(&Xlib, *mut Display) -> Option<T>) -> Option<T> {
    CONN.with(|c| c.as_ref().and_then(|c| f(&c.x, c.d)))
}

fn atom(x: &Xlib, d: *mut Display, name: &str) -> c_ulong {
    let n = CString::new(name).unwrap();
    unsafe { (x.XInternAtom)(d, n.as_ptr(), xlib::False) }
}

/// A 32-bit property (CARDINAL or WINDOW) as its list of values; empty when it is absent
fn prop(x: &Xlib, d: *mut Display, w: c_ulong, name: &str) -> Vec<c_ulong> {
    unsafe {
        let (mut ty, mut fmt, mut n, mut after) = (0, 0 as c_int, 0, 0);
        let mut data: *mut c_uchar = std::ptr::null_mut();
        let ok = (x.XGetWindowProperty)(
            d, w, atom(x, d, name), 0, 4096, xlib::False, xlib::AnyPropertyType as c_ulong,
            &mut ty, &mut fmt, &mut n, &mut after, &mut data,
        );
        if ok != xlib::Success as c_int || data.is_null() {
            return vec![];
        }
        // Format 32 comes back as an array of C longs, whatever the word size
        let v = if fmt == 32 { std::slice::from_raw_parts(data as *const c_ulong, n as usize).to_vec() } else { vec![] };
        (x.XFree)(data as *mut _);
        v
    }
}

fn pid_of(x: &Xlib, d: *mut Display, w: c_ulong) -> u32 {
    prop(x, d, w, "_NET_WM_PID").first().copied().unwrap_or(0) as u32
}

pub fn left_button_down() -> bool {
    with(|x, d| unsafe {
        let root = (x.XDefaultRootWindow)(d);
        let (mut r, mut c, mut rx, mut ry, mut wx, mut wy, mut mask) = (0, 0, 0, 0, 0, 0, 0 as c_uint);
        let ok = (x.XQueryPointer)(d, root, &mut r, &mut c, &mut rx, &mut ry, &mut wx, &mut wy, &mut mask) != 0;
        Some(ok && mask & xlib::Button1Mask != 0)
    })
    .unwrap_or(false)
}

/// The process owning the window the window manager calls active; 0 when unknown
pub fn active_pid() -> u32 {
    with(|x, d| {
        let root = unsafe { (x.XDefaultRootWindow)(d) };
        let w = *prop(x, d, root, "_NET_ACTIVE_WINDOW").first()?;
        (w != 0).then(|| pid_of(x, d, w))
    })
    .unwrap_or(0)
}

/// The window manager's top-level windows, each with the process that owns it
pub fn client_windows() -> Vec<(u64, u32)> {
    with(|x, d| {
        let root = unsafe { (x.XDefaultRootWindow)(d) };
        Some(prop(x, d, root, "_NET_CLIENT_LIST").into_iter().map(|w| (w as u64, pid_of(x, d, w))).collect())
    })
    .unwrap_or_default()
}

/// Asks the window manager to restore, raise and focus a window (EWMH _NET_ACTIVE_WINDOW,
/// sent as a pager — source 2 — which window managers honour without focus-stealing checks)
pub fn activate(win: u64) -> bool {
    with(|x, d| unsafe {
        let root = (x.XDefaultRootWindow)(d);
        let mut ev: xlib::XEvent = std::mem::zeroed();
        ev.client_message = xlib::XClientMessageEvent {
            type_: xlib::ClientMessage,
            serial: 0,
            send_event: xlib::True,
            display: d,
            window: win as c_ulong,
            message_type: atom(x, d, "_NET_ACTIVE_WINDOW"),
            format: 32,
            data: xlib::ClientMessageData::from([2 as c_long, xlib::CurrentTime as c_long, 0, 0, 0]),
        };
        let sent = (x.XSendEvent)(d, root, xlib::False, xlib::SubstructureRedirectMask | xlib::SubstructureNotifyMask, &mut ev) != 0;
        (x.XFlush)(d);
        Some(sent)
    })
    .unwrap_or(false)
}
