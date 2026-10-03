//! Guest-side bindings for Acid OS WASM carts: ABI version 1, import module
//! `"acid"` (spec §15.3).
//!
//! A cart implements [`Cart`] and names its type in [`acid_cart!`], which
//! defines every export the host calls. The host owns the event loop
//! (§15.3), so a cart never polls: it reacts to `on_event` and `on_idle`.
//!
//! The safe wrappers below take Rust strings and slices and hand the host
//! `(ptr, len)` pairs. Results that are strings or records are written into
//! a caller-supplied buffer; the call returns the full length, which may be
//! larger than the buffer (the result is then truncated to fit).

#![no_std]

/// The ABI version this crate speaks; `acid_abi_version` returns it.
pub const ABI_VERSION: i32 = 1;

/// The raw imports, exactly as in the ABI v1 table. Prefer the safe
/// wrappers at the crate root.
pub mod sys {
    #[link(wasm_import_module = "acid")]
    unsafe extern "C" {
        pub fn now_ms() -> i64;
        pub fn notify_redraw_done();
        pub fn fill_rect(x: i32, y: i32, w: i32, h: i32, color: i32);
        pub fn fill_circle(x: i32, y: i32, r: i32, color: i32);
        pub fn draw_text(ptr: *const u8, len: i32, x: i32, y: i32, fg: i32, bg: i32);
        pub fn draw_window_frame(ptr: *const u8, len: i32);
        pub fn draw_window_border();
        pub fn clear_user_area();
        pub fn am_i_focused() -> i32;
        pub fn launch_arg(buf: *mut u8, cap: i32) -> i32;

        pub fn play_note(voice: i32, ona: i32, volume: i32);
        pub fn stop_note(voice: i32);
        pub fn configure_voice(voice: i32, route: i32, attack: i32, decay: i32, sustain: i32, release: i32);
        pub fn configure_filter(cutoff: i32, resonance: i32, mode: i32);
        pub fn trigger_arp(voice: i32, n0: i32, n1: i32, n2: i32, n3: i32, count: i32, rate_ms: i32);
        pub fn configure_osc(voice: i32, waveform: i32, duty: i32);
        pub fn set_ring_partner(voice: i32, partner: i32);
        pub fn set_volume(percent: i32);
        pub fn get_volume() -> i32;
        pub fn active_voice_count() -> i32;

        pub fn overlay_open() -> i32;
        pub fn overlay_clear();
        pub fn overlay_fill_rect(x: i32, y: i32, w: i32, h: i32, color: i32);
        pub fn overlay_close();
        pub fn repaint_region(x: i32, y: i32, w: i32, h: i32);
        pub fn set_wallpaper_enabled(on: i32);
        pub fn get_wallpaper_enabled() -> i32;

        pub fn window_max() -> i32;
        pub fn window_info(index: i32, buf: *mut u8, cap: i32) -> i32;
        pub fn activate_window(index: i32);
        pub fn close_window(index: i32) -> i32;
        pub fn send_self_to_back();

        pub fn launcher_register(
            path_ptr: *const u8,
            path_len: i32,
            name_ptr: *const u8,
            name_len: i32,
            w: i32,
            h: i32,
            multi: i32,
            libs_ptr: *const u8,
            libs_len: i32,
        ) -> i32;
        pub fn launcher_count() -> i32;
        pub fn launcher_path(index: i32, buf: *mut u8, cap: i32) -> i32;
        pub fn launcher_name(index: i32, buf: *mut u8, cap: i32) -> i32;
        pub fn launcher_spawn(index: i32) -> i32;
        pub fn spawn_app(path_ptr: *const u8, path_len: i32, w: i32, h: i32, arg_ptr: *const u8, arg_len: i32) -> i32;
        pub fn composited_frames() -> i32;
        pub fn skipped_frames() -> i32;

        pub fn local_time(buf: *mut u8, cap: i32) -> i32;
        pub fn mem_used_kb() -> i64;
        pub fn network_info(buf: *mut u8, cap: i32) -> i32;
        pub fn refresh_tasks() -> i32;
        pub fn task_count() -> i32;
        pub fn task_info(index: i32, buf: *mut u8, cap: i32) -> i32;

        pub fn fs_list(ptr: *const u8, len: i32, buf: *mut u8, cap: i32) -> i32;
        pub fn fs_read(ptr: *const u8, len: i32, buf: *mut u8, cap: i32) -> i32;
        pub fn fs_size(ptr: *const u8, len: i32) -> i64;
        pub fn fs_write(ptr: *const u8, len: i32, data_ptr: *const u8, data_len: i32) -> i32;
        pub fn fs_rename(from_ptr: *const u8, from_len: i32, to_ptr: *const u8, to_len: i32) -> i32;
        pub fn fs_delete(ptr: *const u8, len: i32) -> i32;

        pub fn cart_roots(buf: *mut u8, cap: i32) -> i32;
        pub fn cart_list(ptr: *const u8, len: i32, buf: *mut u8, cap: i32) -> i32;
        pub fn cart_stat(ptr: *const u8, len: i32, buf: *mut u8, cap: i32) -> i32;
        pub fn cart_read(ptr: *const u8, len: i32, buf: *mut u8, cap: i32) -> i32;
    }
}

/// A failed file or cart-folder call (§15.3's error codes).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    /// −1
    NotFound,
    /// −2
    BadPath,
    /// −3
    ReadOnly,
    /// −4: a cart may not do this (carts are always cart-level, §14.2).
    NotAllowed,
    /// −5
    TooBig,
    /// −6, or any code this version doesn't know.
    Other,
}

impl Error {
    /// Maps a negative ABI result to an error.
    pub fn from_code(code: i64) -> Self {
        match code {
            -1 => Error::NotFound,
            -2 => Error::BadPath,
            -3 => Error::ReadOnly,
            -4 => Error::NotAllowed,
            -5 => Error::TooBig,
            _ => Error::Other,
        }
    }
}

/// An input event, decoded from `acid_on_event(kind, a, b, c)` (§15.3).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Event {
    Touch { x: i32, y: i32, pressed: bool },
    Key { code: i32, pressed: bool },
    /// The window moved; the host calls `redraw` right after this.
    Moved,
    /// The window is closing; `on_destroy` follows, then the cart ends.
    Close,
}

impl Event {
    /// Decodes the export's arguments; `None` for a kind this ABI doesn't define.
    pub fn decode(kind: i32, a: i32, b: i32, c: i32) -> Option<Self> {
        match kind {
            1 => Some(Event::Touch { x: a, y: b, pressed: c != 0 }),
            2 => Some(Event::Key { code: a, pressed: b != 0 }),
            3 => Some(Event::Moved),
            4 => Some(Event::Close),
            _ => None,
        }
    }
}

/// A cart. The host calls `new` then `on_create` once, then `redraw`, then
/// `on_event` / `on_idle` until close (§15.3).
pub trait Cart: Sized + 'static {
    fn new() -> Self;
    fn on_create(&mut self) {}
    fn on_event(&mut self, _e: Event) {}
    /// Each poll timeout with no event.
    fn on_idle(&mut self) {}
    fn redraw(&mut self);
    /// How long the host waits for an event before `on_idle`; values below
    /// 1 count as 1.
    fn poll_timeout_ms(&self) -> i32 {
        200
    }
    fn on_destroy(&mut self) {}
}

// Lengths cross the ABI as i32. A wasm32 slice can't exceed 4 GiB, and one
// over 2 GiB is clamped: the host then reads or writes less, never past it.
fn len32(len: usize) -> i32 {
    i32::try_from(len).unwrap_or(i32::MAX)
}

fn len_or_err(r: i32) -> Result<usize, Error> {
    if r < 0 { Err(Error::from_code(i64::from(r))) } else { Ok(r as usize) }
}

fn unit_or_err(r: i32) -> Result<(), Error> {
    len_or_err(r).map(|_| ())
}

// −1 means "no such thing" for indexed records (§15.3).
fn len_opt(r: i32) -> Option<usize> {
    if r < 0 { None } else { Some(r as usize) }
}

// ---- Drawing and the window ------------------------------------------------

pub fn now_ms() -> i64 {
    unsafe { sys::now_ms() }
}

pub fn notify_redraw_done() {
    unsafe { sys::notify_redraw_done() }
}

/// Colours are 0xRRGGBB.
pub fn fill_rect(x: i32, y: i32, w: i32, h: i32, color: u32) {
    unsafe { sys::fill_rect(x, y, w, h, color as i32) }
}

pub fn fill_circle(x: i32, y: i32, r: i32, color: u32) {
    unsafe { sys::fill_circle(x, y, r, color as i32) }
}

pub fn draw_text(text: &str, x: i32, y: i32, fg: u32, bg: u32) {
    unsafe { sys::draw_text(text.as_ptr(), len32(text.len()), x, y, fg as i32, bg as i32) }
}

pub fn draw_window_frame(title: &str) {
    unsafe { sys::draw_window_frame(title.as_ptr(), len32(title.len())) }
}

pub fn draw_window_border() {
    unsafe { sys::draw_window_border() }
}

pub fn clear_user_area() {
    unsafe { sys::clear_user_area() }
}

pub fn focused() -> bool {
    unsafe { sys::am_i_focused() != 0 }
}

/// The argument this cart was spawned with ("" if none); returns its full length.
pub fn launch_arg(buf: &mut [u8]) -> usize {
    unsafe { sys::launch_arg(buf.as_mut_ptr(), len32(buf.len())).max(0) as usize }
}

// ---- Audio ------------------------------------------------------------------

pub fn play_note(voice: i32, ona: i32, volume: i32) {
    unsafe { sys::play_note(voice, ona, volume) }
}

pub fn stop_note(voice: i32) {
    unsafe { sys::stop_note(voice) }
}

pub fn configure_voice(voice: i32, route: i32, attack: i32, decay: i32, sustain: i32, release: i32) {
    unsafe { sys::configure_voice(voice, route, attack, decay, sustain, release) }
}

pub fn configure_filter(cutoff: i32, resonance: i32, mode: i32) {
    unsafe { sys::configure_filter(cutoff, resonance, mode) }
}

pub fn trigger_arp(voice: i32, notes: [i32; 4], count: i32, rate_ms: i32) {
    unsafe { sys::trigger_arp(voice, notes[0], notes[1], notes[2], notes[3], count, rate_ms) }
}

pub fn configure_osc(voice: i32, waveform: i32, duty: i32) {
    unsafe { sys::configure_osc(voice, waveform, duty) }
}

pub fn set_ring_partner(voice: i32, partner: i32) {
    unsafe { sys::set_ring_partner(voice, partner) }
}

pub fn set_volume(percent: i32) {
    unsafe { sys::set_volume(percent) }
}

pub fn get_volume() -> i32 {
    unsafe { sys::get_volume() }
}

pub fn active_voice_count() -> i32 {
    unsafe { sys::active_voice_count() }
}

// ---- Overlay and wallpaper --------------------------------------------------

pub fn overlay_open() -> bool {
    unsafe { sys::overlay_open() != 0 }
}

pub fn overlay_clear() {
    unsafe { sys::overlay_clear() }
}

pub fn overlay_fill_rect(x: i32, y: i32, w: i32, h: i32, color: u32) {
    unsafe { sys::overlay_fill_rect(x, y, w, h, color as i32) }
}

pub fn overlay_close() {
    unsafe { sys::overlay_close() }
}

pub fn repaint_region(x: i32, y: i32, w: i32, h: i32) {
    unsafe { sys::repaint_region(x, y, w, h) }
}

pub fn set_wallpaper_enabled(on: bool) {
    unsafe { sys::set_wallpaper_enabled(i32::from(on)) }
}

pub fn get_wallpaper_enabled() -> bool {
    unsafe { sys::get_wallpaper_enabled() != 0 }
}

// ---- Windows ----------------------------------------------------------------

pub fn window_max() -> i32 {
    unsafe { sys::window_max() }
}

/// Writes the record `name\tx\ty\tw\th\tfocused`; `None` if no window has
/// that index.
pub fn window_info(index: i32, buf: &mut [u8]) -> Option<usize> {
    len_opt(unsafe { sys::window_info(index, buf.as_mut_ptr(), len32(buf.len())) })
}

pub fn activate_window(index: i32) {
    unsafe { sys::activate_window(index) }
}

/// True only when a window was closed. A cart is always refused (the host
/// answers −4), so a cart always gets false (§16.2).
pub fn close_window(index: i32) -> bool {
    unsafe { sys::close_window(index) == 1 }
}

pub fn send_self_to_back() {
    unsafe { sys::send_self_to_back() }
}

// ---- Launcher ---------------------------------------------------------------

/// Always false for a WASM cart, which is cart-level (§14.2); kept so the
/// bindings mirror the Lua calls.
pub fn launcher_register(path: &str, name: &str, w: i32, h: i32, multi: bool, libs: &str) -> bool {
    unsafe {
        sys::launcher_register(
            path.as_ptr(),
            len32(path.len()),
            name.as_ptr(),
            len32(name.len()),
            w,
            h,
            i32::from(multi),
            libs.as_ptr(),
            len32(libs.len()),
        ) != 0
    }
}

pub fn launcher_count() -> i32 {
    unsafe { sys::launcher_count() }
}

pub fn launcher_path(index: i32, buf: &mut [u8]) -> Option<usize> {
    len_opt(unsafe { sys::launcher_path(index, buf.as_mut_ptr(), len32(buf.len())) })
}

pub fn launcher_name(index: i32, buf: &mut [u8]) -> Option<usize> {
    len_opt(unsafe { sys::launcher_name(index, buf.as_mut_ptr(), len32(buf.len())) })
}

pub fn launcher_spawn(index: i32) -> bool {
    unsafe { sys::launcher_spawn(index) != 0 }
}

/// `arg` "" means no argument.
pub fn spawn_app(path: &str, w: i32, h: i32, arg: &str) -> bool {
    unsafe { sys::spawn_app(path.as_ptr(), len32(path.len()), w, h, arg.as_ptr(), len32(arg.len())) != 0 }
}

pub fn composited_frames() -> i32 {
    unsafe { sys::composited_frames() }
}

pub fn skipped_frames() -> i32 {
    unsafe { sys::skipped_frames() }
}

// ---- System -----------------------------------------------------------------

/// Writes the record `year\tmonth\tday\thour\tmin\tsec`.
pub fn local_time(buf: &mut [u8]) -> usize {
    unsafe { sys::local_time(buf.as_mut_ptr(), len32(buf.len())).max(0) as usize }
}

pub fn mem_used_kb() -> i64 {
    unsafe { sys::mem_used_kb() }
}

/// Writes the record `host\tip\tconnected`.
pub fn network_info(buf: &mut [u8]) -> usize {
    unsafe { sys::network_info(buf.as_mut_ptr(), len32(buf.len())).max(0) as usize }
}

pub fn refresh_tasks() -> i32 {
    unsafe { sys::refresh_tasks() }
}

pub fn task_count() -> i32 {
    unsafe { sys::task_count() }
}

/// Writes the record `name\tstate\tcpu`; `None` if there is no such task.
pub fn task_info(index: i32, buf: &mut [u8]) -> Option<usize> {
    len_opt(unsafe { sys::task_info(index, buf.as_mut_ptr(), len32(buf.len())) })
}

// ---- Files ------------------------------------------------------------------

/// Names joined by `\n`.
pub fn fs_list(path: &str, buf: &mut [u8]) -> Result<usize, Error> {
    len_or_err(unsafe { sys::fs_list(path.as_ptr(), len32(path.len()), buf.as_mut_ptr(), len32(buf.len())) })
}

/// The file's raw bytes; returns the full size, which may exceed `buf`.
pub fn fs_read(path: &str, buf: &mut [u8]) -> Result<usize, Error> {
    len_or_err(unsafe { sys::fs_read(path.as_ptr(), len32(path.len()), buf.as_mut_ptr(), len32(buf.len())) })
}

pub fn fs_size(path: &str) -> Result<u64, Error> {
    let r = unsafe { sys::fs_size(path.as_ptr(), len32(path.len())) };
    if r < 0 { Err(Error::from_code(r)) } else { Ok(r as u64) }
}

/// A cart may write only under `v3/fsroot/Home` (§14.2).
pub fn fs_write(path: &str, data: &[u8]) -> Result<(), Error> {
    unit_or_err(unsafe { sys::fs_write(path.as_ptr(), len32(path.len()), data.as_ptr(), len32(data.len())) })
}

pub fn fs_rename(from: &str, to: &str) -> Result<(), Error> {
    unit_or_err(unsafe { sys::fs_rename(from.as_ptr(), len32(from.len()), to.as_ptr(), len32(to.len())) })
}

pub fn fs_delete(path: &str) -> Result<(), Error> {
    unit_or_err(unsafe { sys::fs_delete(path.as_ptr(), len32(path.len())) })
}

// ---- Host cart folders (always NotAllowed for a cart, §14.2) ----------------

pub fn cart_roots(buf: &mut [u8]) -> Result<usize, Error> {
    len_or_err(unsafe { sys::cart_roots(buf.as_mut_ptr(), len32(buf.len())) })
}

pub fn cart_list(path: &str, buf: &mut [u8]) -> Result<usize, Error> {
    len_or_err(unsafe { sys::cart_list(path.as_ptr(), len32(path.len()), buf.as_mut_ptr(), len32(buf.len())) })
}

/// Writes the record `dir|file\tsize`.
pub fn cart_stat(path: &str, buf: &mut [u8]) -> Result<usize, Error> {
    len_or_err(unsafe { sys::cart_stat(path.as_ptr(), len32(path.len()), buf.as_mut_ptr(), len32(buf.len())) })
}

pub fn cart_read(path: &str, buf: &mut [u8]) -> Result<usize, Error> {
    len_or_err(unsafe { sys::cart_read(path.as_ptr(), len32(path.len()), buf.as_mut_ptr(), len32(buf.len())) })
}

// ---- Panics -----------------------------------------------------------------

// A panic traps the cart; the host ends it and removes its window (§15.2).
// `unreachable` is the wasm trap instruction, so this never returns.
#[cfg(target_arch = "wasm32")]
#[panic_handler]
fn panic(_: &core::panic::PanicInfo) -> ! {
    loop {
        core::arch::wasm32::unreachable();
    }
}

/// Defines the cart's exports (§15.3) for a type implementing [`Cart`]:
/// `acid_abi_version`, `acid_on_create`, `acid_on_event`, `acid_on_idle`,
/// `acid_redraw`, `acid_poll_timeout_ms` and `acid_on_destroy`. Use it once,
/// at the cart crate's root.
#[macro_export]
macro_rules! acid_cart {
    ($t:ty) => {
        // The cart instance lives in a `static mut`. That is sound here: a
        // wasm32 module without threads has exactly one thread, and the host
        // calls one export at a time and never re-enters the cart from an
        // import, so no two references to it are ever live at once. It is
        // reached only through raw pointers (`&raw mut`), never `&STATIC`.
        static mut __ACID_CART: ::core::option::Option<$t> = ::core::option::Option::None;
        // Set by a "moved" event: the host calls acid_redraw next, after
        // which the redraw is reported done, as AcidApp:start does in Lua.
        static mut __ACID_MOVED: bool = false;

        fn __acid_cart() -> ::core::option::Option<&'static mut $t> {
            unsafe { (*&raw mut __ACID_CART).as_mut() }
        }

        #[unsafe(no_mangle)]
        pub extern "C" fn acid_abi_version() -> i32 {
            $crate::ABI_VERSION
        }

        #[unsafe(no_mangle)]
        pub extern "C" fn acid_on_create() {
            unsafe { *&raw mut __ACID_CART = ::core::option::Option::Some(<$t as $crate::Cart>::new()) };
            if let ::core::option::Option::Some(c) = __acid_cart() {
                $crate::Cart::on_create(c);
            }
        }

        #[unsafe(no_mangle)]
        pub extern "C" fn acid_on_event(kind: i32, a: i32, b: i32, c: i32) {
            let ::core::option::Option::Some(e) = $crate::Event::decode(kind, a, b, c) else { return };
            if e == $crate::Event::Moved {
                unsafe { *&raw mut __ACID_MOVED = true };
            }
            if let ::core::option::Option::Some(cart) = __acid_cart() {
                $crate::Cart::on_event(cart, e);
            }
        }

        #[unsafe(no_mangle)]
        pub extern "C" fn acid_on_idle() {
            if let ::core::option::Option::Some(c) = __acid_cart() {
                $crate::Cart::on_idle(c);
            }
        }

        #[unsafe(no_mangle)]
        pub extern "C" fn acid_redraw() {
            if let ::core::option::Option::Some(c) = __acid_cart() {
                $crate::Cart::redraw(c);
            }
            if unsafe { ::core::mem::replace(&mut *&raw mut __ACID_MOVED, false) } {
                $crate::notify_redraw_done();
            }
        }

        #[unsafe(no_mangle)]
        pub extern "C" fn acid_poll_timeout_ms() -> i32 {
            match __acid_cart() {
                ::core::option::Option::Some(c) => $crate::Cart::poll_timeout_ms(c),
                ::core::option::Option::None => 200,
            }
        }

        #[unsafe(no_mangle)]
        pub extern "C" fn acid_on_destroy() {
            if let ::core::option::Option::Some(c) = __acid_cart() {
                $crate::Cart::on_destroy(c);
            }
            // Drops the cart: the host ends it after this call.
            unsafe { *&raw mut __ACID_CART = ::core::option::Option::None };
        }
    };
}
