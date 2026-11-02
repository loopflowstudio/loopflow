//! The terminal a Session owner keeps: libghostty-vt behind a safe handle.
//!
//! `build.rs` links the prebuilt `libghostty-vt.a` pinned in `ghostty-vt.json`.
//! A [`Terminal`] parses the program's output, answers its queries, repaints a
//! new viewer through Ghostty's VT formatter, and encodes or decodes the
//! Ghostty snapshot format (`GHOSTSNP`). Snapshot format v1 carries no
//! compatibility guarantee, so a persisted snapshot names the Ghostty revision
//! that wrote it and reads as absent under any other.

use std::ffi::c_void;
use std::fmt;

/// The Ghostty revision the linked library was built from.
pub const REVISION: &str = env!("LOOPFLOW_GHOSTTY_REVISION");

/// How much of the program's unfinished escape sequence the owner retains for
/// a reconnecting viewer.
const CONTINUATION_LIMIT: usize = 64 * 1024;

/// Scrollback the owner keeps. Ghostty prunes whole pages of about 400 KiB.
const SCROLLBACK_LIMIT: usize = 8 * 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Error {
    call: &'static str,
    code: i32,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = match self.code {
            ffi::OUT_OF_MEMORY => "out of memory",
            ffi::INVALID_VALUE => "invalid value",
            ffi::OUT_OF_SPACE => "out of space",
            ffi::NO_VALUE => "no value",
            ffi::IO_ERROR => "I/O error",
            ffi::LIMIT_EXCEEDED => "limit exceeded",
            ffi::REJECTED => "rejected",
            _ => "unknown error",
        };
        write!(f, "libghostty-vt {}: {name} ({})", self.call, self.code)
    }
}

impl std::error::Error for Error {}

fn check(call: &'static str, code: i32) -> Result<(), Error> {
    if code == ffi::SUCCESS {
        Ok(())
    } else {
        Err(Error { call, code })
    }
}

/// Bytes the terminal wrote back toward the program: DSR, DA and mode reports.
#[derive(Debug, Default)]
struct Replies(Vec<u8>);

unsafe extern "C" fn write_pty(
    _terminal: ffi::Terminal,
    userdata: *mut c_void,
    data: *const u8,
    len: usize,
) {
    // SAFETY: userdata is the Box<Replies> this terminal registered and owns
    // for its whole life; data points to len readable bytes for this call.
    unsafe {
        let replies = &mut *userdata.cast::<Replies>();
        replies
            .0
            .extend_from_slice(std::slice::from_raw_parts(data, len));
    }
}

unsafe extern "C" fn device_attributes(
    _terminal: ffi::Terminal,
    _userdata: *mut c_void,
    out: *mut ffi::DeviceAttributes,
) -> bool {
    // SAFETY: the library passes a writable, correctly sized struct.
    unsafe {
        // What Ghostty answers: VT220 conformance with ANSI color (DA1),
        // a VT220 at firmware 10 (DA2), unit 0 (DA3).
        let attrs = &mut *out;
        attrs.primary.conformance_level = 62;
        attrs.primary.features[0] = 22;
        attrs.primary.num_features = 1;
        attrs.secondary.device_type = 1;
        attrs.secondary.firmware_version = 10;
        attrs.secondary.rom_cartridge = 0;
        attrs.tertiary.unit_id = 0;
    }
    true
}

/// One libghostty-vt terminal. Every call is serialized through `&mut self`;
/// callbacks run inside `write`.
pub struct Terminal {
    raw: ffi::Terminal,
    replies: Box<Replies>,
}

impl fmt::Debug for Terminal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let (cols, rows) = self.size();
        f.debug_struct("Terminal")
            .field("cols", &cols)
            .field("rows", &rows)
            .finish()
    }
}

// SAFETY: the handle is only reached through this type's exclusive methods, and
// the library keeps no thread-affine state behind it.
unsafe impl Send for Terminal {}

impl Terminal {
    pub fn new(cols: u16, rows: u16) -> Result<Self, Error> {
        let mut raw: ffi::Terminal = std::ptr::null_mut();
        // SAFETY: raw receives a handle or stays NULL on error.
        check("terminal_new", unsafe {
            ffi::ghostty_terminal_new(std::ptr::null(), &mut raw, cols, rows)
        })?;
        let mut terminal = Self {
            raw,
            replies: Box::default(),
        };
        terminal.configure()?;
        Ok(terminal)
    }

    /// Callbacks and limits every owner terminal uses, including one restored
    /// from a snapshot.
    fn configure(&mut self) -> Result<(), Error> {
        let userdata: *mut Replies = &mut *self.replies;
        // Callback options take the function pointer itself as the value.
        let write: ffi::WritePtyFn = write_pty;
        let attributes: ffi::DeviceAttributesFn = device_attributes;
        // SAFETY: each value pointer has the type its option documents and
        // outlives the call; the userdata Box lives as long as the terminal.
        unsafe {
            self.set("userdata", ffi::OPT_USERDATA, userdata.cast())?;
            self.set("write_pty", ffi::OPT_WRITE_PTY, write as *const c_void)?;
            self.set(
                "device_attributes",
                ffi::OPT_DEVICE_ATTRIBUTES,
                attributes as *const c_void,
            )?;
            self.set(
                "scrollback",
                ffi::OPT_SCROLLBACK_MAX_BYTES,
                (&SCROLLBACK_LIMIT as *const usize).cast(),
            )?;
            self.set(
                "continuation",
                ffi::OPT_CONTINUATION_MAX_BYTES,
                (&CONTINUATION_LIMIT as *const usize).cast(),
            )
        }
    }

    /// # Safety
    /// `value` must point at the type `option` documents and outlive the call.
    unsafe fn set(
        &self,
        call: &'static str,
        option: i32,
        value: *const c_void,
    ) -> Result<(), Error> {
        // SAFETY: as documented by the caller.
        check(call, unsafe {
            ffi::ghostty_terminal_set(self.raw, option, value)
        })
    }

    pub fn write(&mut self, bytes: &[u8]) {
        if bytes.is_empty() {
            return;
        }
        // SAFETY: bytes is a readable slice for the duration of the call.
        unsafe { ffi::ghostty_terminal_vt_write(self.raw, bytes.as_ptr(), bytes.len()) }
    }

    /// Replies the program is owed since the last call.
    pub fn take_replies(&mut self) -> Vec<u8> {
        std::mem::take(&mut self.replies.0)
    }

    pub fn resize(&mut self, cols: u16, rows: u16) -> Result<(), Error> {
        // SAFETY: a live handle; pixel sizes are unused without images.
        check("terminal_resize", unsafe {
            ffi::ghostty_terminal_resize(self.raw, cols, rows, 0, 0)
        })
    }

    fn data<T: Default>(&self, key: i32, call: &'static str) -> Result<T, Error> {
        let mut out = T::default();
        // SAFETY: out has the output type the data key documents.
        check(call, unsafe {
            ffi::ghostty_terminal_get(self.raw, key, (&mut out as *mut T).cast())
        })?;
        Ok(out)
    }

    pub fn size(&self) -> (u16, u16) {
        let cols = self.data::<u16>(ffi::DATA_COLS, "cols").unwrap_or(0);
        let rows = self.data::<u16>(ffi::DATA_ROWS, "rows").unwrap_or(0);
        (cols, rows)
    }

    pub fn cursor(&self) -> (u16, u16) {
        let x = self
            .data::<u16>(ffi::DATA_CURSOR_X, "cursor_x")
            .unwrap_or(0);
        let y = self
            .data::<u16>(ffi::DATA_CURSOR_Y, "cursor_y")
            .unwrap_or(0);
        (x, y)
    }

    pub fn alternate_screen(&self) -> bool {
        self.data::<i32>(ffi::DATA_ACTIVE_SCREEN, "active_screen")
            .is_ok_and(|screen| screen == ffi::SCREEN_ALTERNATE)
    }

    /// Rows of the active screen's scrollback above its visible area.
    pub fn scrollback_rows(&self) -> usize {
        self.data::<usize>(ffi::DATA_SCROLLBACK_ROWS, "scrollback_rows")
            .unwrap_or(0)
    }

    /// Whether the parser is between sequences, so inserted bytes cannot split one.
    pub fn at_ground(&self) -> bool {
        self.data::<bool>(ffi::DATA_VT_GROUND, "vt_ground")
            .unwrap_or(true)
    }

    /// The unfinished escape sequence or UTF-8 character the program is mid-way
    /// through, so a viewer attached now can finish it.
    pub fn continuation(&self) -> Result<Vec<u8>, Error> {
        let mut ptr: *mut u8 = std::ptr::null_mut();
        let mut len = 0usize;
        // SAFETY: on success ptr/len describe an allocation this function frees.
        unsafe {
            check(
                "terminal_continuation",
                ffi::ghostty_terminal_continuation_alloc(
                    self.raw,
                    std::ptr::null(),
                    &mut ptr,
                    &mut len,
                ),
            )?;
            Ok(take_allocation(ptr, len))
        }
    }

    /// The active screen's visible rows as plain text, one line per row.
    pub fn text(&self) -> Result<String, Error> {
        let bytes = self.format(ffi::FORMAT_PLAIN, Range::Active, false)?;
        Ok(String::from_utf8_lossy(&bytes).into_owned())
    }

    /// VT bytes that draw the active screen from the cursor's home position and
    /// restore every state the formatter can express: styles, hyperlinks, cursor,
    /// Kitty keyboard flags, charsets, palette, modes (alternate screen, mouse,
    /// bracketed paste), scrolling region, tabstops and pwd.
    pub fn repaint_screen(&self) -> Result<Vec<u8>, Error> {
        self.format(ffi::FORMAT_VT, Range::Active, true)
    }

    /// VT bytes that print the newest `rows` scrollback rows of the active
    /// screen, oldest first, with their styles and hyperlinks.
    pub fn repaint_history(&self, rows: usize) -> Result<Vec<u8>, Error> {
        let available = self.scrollback_rows();
        let rows = rows.min(available);
        if rows == 0 {
            return Ok(Vec::new());
        }
        self.format(
            ffi::FORMAT_VT,
            Range::History {
                first: available - rows,
                last: available - 1,
            },
            false,
        )
    }

    /// `restore` adds the escape sequences that put a terminal in the owner's
    /// state (cursor, styles, hyperlinks, keyboard, charsets, modes, scrolling
    /// region, tabstops, pwd) around the formatted rows.
    fn format(&self, emit: i32, range: Range, restore: bool) -> Result<Vec<u8>, Error> {
        let (cols, rows) = self.size();
        let (start, end) = match range {
            Range::Active => (
                self.grid_ref(ffi::POINT_TAG_ACTIVE, 0, 0)?,
                self.grid_ref(ffi::POINT_TAG_ACTIVE, cols - 1, u32::from(rows) - 1)?,
            ),
            Range::History { first, last } => (
                self.grid_ref(ffi::POINT_TAG_HISTORY, 0, first as u32)?,
                self.grid_ref(ffi::POINT_TAG_HISTORY, cols - 1, last as u32)?,
            ),
        };
        let selection = ffi::Selection {
            size: std::mem::size_of::<ffi::Selection>(),
            start,
            end,
            rectangle: false,
        };
        let options = ffi::FormatterTerminalOptions {
            size: std::mem::size_of::<ffi::FormatterTerminalOptions>(),
            emit,
            unwrap: false,
            trim: false,
            extra: ffi::FormatterTerminalExtra {
                size: std::mem::size_of::<ffi::FormatterTerminalExtra>(),
                // The viewer's terminal keeps its own palette; the owner
                // never learned the real one.
                palette: false,
                modes: restore,
                scrolling_region: restore,
                tabstops: restore,
                pwd: restore,
                keyboard: restore,
                screen: ffi::FormatterScreenExtra {
                    size: std::mem::size_of::<ffi::FormatterScreenExtra>(),
                    cursor: restore,
                    style: restore,
                    hyperlink: restore,
                    protection: restore,
                    kitty_keyboard: restore,
                    charsets: restore,
                },
            },
            selection: &selection,
        };
        let mut formatter: ffi::Formatter = std::ptr::null_mut();
        // SAFETY: the terminal and selection outlive the formatter, which is
        // freed before this function returns; ptr/len describe an allocation
        // this function frees.
        unsafe {
            check(
                "formatter_terminal_new",
                ffi::ghostty_formatter_terminal_new(
                    std::ptr::null(),
                    &mut formatter,
                    self.raw,
                    options,
                ),
            )?;
            let mut ptr: *mut u8 = std::ptr::null_mut();
            let mut len = 0usize;
            let result = ffi::ghostty_formatter_format_alloc(
                formatter,
                std::ptr::null(),
                &mut ptr,
                &mut len,
            );
            ffi::ghostty_formatter_free(formatter);
            check("formatter_format", result)?;
            Ok(take_allocation(ptr, len))
        }
    }

    fn grid_ref(&self, tag: i32, x: u16, y: u32) -> Result<ffi::GridRef, Error> {
        let point = ffi::Point {
            tag,
            value: ffi::PointValue {
                coordinate: ffi::PointCoordinate { x, y },
            },
        };
        let mut reference = ffi::GridRef {
            size: std::mem::size_of::<ffi::GridRef>(),
            node: std::ptr::null_mut(),
            x: 0,
            y: 0,
        };
        // SAFETY: reference is a writable struct of the declared size.
        check("terminal_grid_ref", unsafe {
            ffi::ghostty_terminal_grid_ref(self.raw, point, &mut reference)
        })?;
        Ok(reference)
    }

    /// The complete terminal as Ghostty's snapshot stream.
    pub fn encode(&self) -> Result<Vec<u8>, Error> {
        let mut ptr: *mut u8 = std::ptr::null_mut();
        let mut len = 0usize;
        // SAFETY: on success ptr/len describe an allocation this function frees.
        unsafe {
            check(
                "snapshot_encode",
                ffi::ghostty_snapshot_encode_alloc(self.raw, std::ptr::null(), &mut ptr, &mut len),
            )?;
            Ok(take_allocation(ptr, len))
        }
    }

    /// A terminal restored from a snapshot stream, with the owner's callbacks and limits.
    pub fn decode(snapshot: &[u8]) -> Result<Self, Error> {
        let mut decoder: ffi::SnapshotDecoder = std::ptr::null_mut();
        let mut raw: ffi::Terminal = std::ptr::null_mut();
        // SAFETY: the borrowed snapshot bytes outlive the decoder, which is
        // freed before this function returns; raw stays NULL on error.
        unsafe {
            check(
                "snapshot_decoder_new",
                ffi::ghostty_snapshot_decoder_new_buf(
                    std::ptr::null(),
                    &mut decoder,
                    snapshot.as_ptr(),
                    snapshot.len(),
                ),
            )?;
            // Keep the unfinished input the owner had, as a live terminal would.
            let retain = true;
            let result = (|| {
                check(
                    "snapshot_decoder_set retain",
                    ffi::ghostty_snapshot_decoder_set(
                        decoder,
                        ffi::DECODER_OPT_RETAIN_CONTINUATION,
                        (&retain as *const bool).cast(),
                    ),
                )?;
                check(
                    "snapshot_decoder_set continuation",
                    ffi::ghostty_snapshot_decoder_set(
                        decoder,
                        ffi::DECODER_OPT_MAX_CONTINUATION_BYTES,
                        (&CONTINUATION_LIMIT as *const usize).cast(),
                    ),
                )?;
                check(
                    "snapshot_decode",
                    ffi::ghostty_snapshot_decoder_decode(decoder, &mut raw),
                )
            })();
            ffi::ghostty_snapshot_decoder_free(decoder);
            result?;
        }
        let mut terminal = Self {
            raw,
            replies: Box::default(),
        };
        terminal.configure()?;
        Ok(terminal)
    }
}

impl Drop for Terminal {
    fn drop(&mut self) {
        // SAFETY: the handle was returned by the library and is freed once.
        unsafe { ffi::ghostty_terminal_free(self.raw) }
    }
}

/// Copy out and free a buffer the library allocated with its default allocator.
///
/// # Safety
/// `ptr`/`len` must describe one allocation from a `*_alloc` call, or NULL/0.
unsafe fn take_allocation(ptr: *mut u8, len: usize) -> Vec<u8> {
    if ptr.is_null() {
        return Vec::new();
    }
    // SAFETY: as documented by the caller.
    unsafe {
        let bytes = std::slice::from_raw_parts(ptr, len).to_vec();
        ffi::ghostty_free(std::ptr::null(), ptr, len);
        bytes
    }
}

#[derive(Debug, Clone, Copy)]
enum Range {
    Active,
    History { first: usize, last: usize },
}

/// The subset of `include/ghostty/vt/*.h` the owner uses. Enums are `int`
/// (`GHOSTTY_ENUM_TYPED`); structs carry their size first where the header does.
mod ffi {
    use std::ffi::c_void;

    pub type Terminal = *mut c_void;
    pub type Formatter = *mut c_void;
    pub type SnapshotDecoder = *mut c_void;
    pub type Result = i32;

    pub const SUCCESS: Result = 0;
    pub const OUT_OF_MEMORY: Result = -1;
    pub const INVALID_VALUE: Result = -2;
    pub const OUT_OF_SPACE: Result = -3;
    pub const NO_VALUE: Result = -4;
    pub const IO_ERROR: Result = -5;
    pub const LIMIT_EXCEEDED: Result = -6;
    pub const REJECTED: Result = -7;

    pub const OPT_USERDATA: i32 = 0;
    pub const OPT_WRITE_PTY: i32 = 1;
    pub const OPT_DEVICE_ATTRIBUTES: i32 = 8;
    pub const OPT_SCROLLBACK_MAX_BYTES: i32 = 27;
    pub const OPT_CONTINUATION_MAX_BYTES: i32 = 31;

    pub const DATA_COLS: i32 = 1;
    pub const DATA_ROWS: i32 = 2;
    pub const DATA_CURSOR_X: i32 = 3;
    pub const DATA_CURSOR_Y: i32 = 4;
    pub const DATA_ACTIVE_SCREEN: i32 = 6;
    pub const DATA_SCROLLBACK_ROWS: i32 = 15;
    pub const DATA_VT_GROUND: i32 = 38;

    pub const SCREEN_ALTERNATE: i32 = 1;

    pub const DECODER_OPT_MAX_CONTINUATION_BYTES: i32 = 0;
    pub const DECODER_OPT_RETAIN_CONTINUATION: i32 = 1;

    pub const FORMAT_PLAIN: i32 = 0;
    pub const FORMAT_VT: i32 = 1;

    pub const POINT_TAG_ACTIVE: i32 = 0;
    pub const POINT_TAG_HISTORY: i32 = 3;

    pub type WritePtyFn = unsafe extern "C" fn(Terminal, *mut c_void, *const u8, usize);
    pub type DeviceAttributesFn =
        unsafe extern "C" fn(Terminal, *mut c_void, *mut DeviceAttributes) -> bool;

    #[repr(C)]
    pub struct DeviceAttributesPrimary {
        pub conformance_level: u16,
        pub features: [u16; 64],
        pub num_features: usize,
    }

    #[repr(C)]
    pub struct DeviceAttributesSecondary {
        pub device_type: u16,
        pub firmware_version: u16,
        pub rom_cartridge: u16,
    }

    #[repr(C)]
    pub struct DeviceAttributesTertiary {
        pub unit_id: u32,
    }

    #[repr(C)]
    pub struct DeviceAttributes {
        pub primary: DeviceAttributesPrimary,
        pub secondary: DeviceAttributesSecondary,
        pub tertiary: DeviceAttributesTertiary,
    }

    #[repr(C)]
    pub struct FormatterScreenExtra {
        pub size: usize,
        pub cursor: bool,
        pub style: bool,
        pub hyperlink: bool,
        pub protection: bool,
        pub kitty_keyboard: bool,
        pub charsets: bool,
    }

    #[repr(C)]
    pub struct FormatterTerminalExtra {
        pub size: usize,
        pub palette: bool,
        pub modes: bool,
        pub scrolling_region: bool,
        pub tabstops: bool,
        pub pwd: bool,
        pub keyboard: bool,
        pub screen: FormatterScreenExtra,
    }

    #[repr(C)]
    pub struct FormatterTerminalOptions {
        pub size: usize,
        pub emit: i32,
        pub unwrap: bool,
        pub trim: bool,
        pub extra: FormatterTerminalExtra,
        pub selection: *const Selection,
    }

    #[repr(C)]
    pub struct GridRef {
        pub size: usize,
        pub node: *mut c_void,
        pub x: u16,
        pub y: u16,
    }

    #[repr(C)]
    pub struct Selection {
        pub size: usize,
        pub start: GridRef,
        pub end: GridRef,
        pub rectangle: bool,
    }

    #[repr(C)]
    #[derive(Clone, Copy)]
    pub struct PointCoordinate {
        pub x: u16,
        pub y: u32,
    }

    #[repr(C)]
    pub union PointValue {
        pub coordinate: PointCoordinate,
        pub _padding: [u64; 2],
    }

    #[repr(C)]
    pub struct Point {
        pub tag: i32,
        pub value: PointValue,
    }

    extern "C" {
        pub fn ghostty_terminal_new(
            allocator: *const c_void,
            terminal: *mut Terminal,
            cols: u16,
            rows: u16,
        ) -> Result;
        pub fn ghostty_terminal_free(terminal: Terminal);
        pub fn ghostty_terminal_resize(
            terminal: Terminal,
            cols: u16,
            rows: u16,
            cell_width_px: u32,
            cell_height_px: u32,
        ) -> Result;
        pub fn ghostty_terminal_set(
            terminal: Terminal,
            option: i32,
            value: *const c_void,
        ) -> Result;
        pub fn ghostty_terminal_vt_write(terminal: Terminal, data: *const u8, len: usize);
        pub fn ghostty_terminal_continuation_alloc(
            terminal: Terminal,
            allocator: *const c_void,
            out_ptr: *mut *mut u8,
            out_len: *mut usize,
        ) -> Result;
        pub fn ghostty_terminal_get(terminal: Terminal, data: i32, out: *mut c_void) -> Result;
        pub fn ghostty_terminal_grid_ref(
            terminal: Terminal,
            point: Point,
            out_ref: *mut GridRef,
        ) -> Result;
        pub fn ghostty_formatter_terminal_new(
            allocator: *const c_void,
            formatter: *mut Formatter,
            terminal: Terminal,
            options: FormatterTerminalOptions,
        ) -> Result;
        pub fn ghostty_formatter_format_alloc(
            formatter: Formatter,
            allocator: *const c_void,
            out_ptr: *mut *mut u8,
            out_len: *mut usize,
        ) -> Result;
        pub fn ghostty_formatter_free(formatter: Formatter);
        pub fn ghostty_snapshot_encode_alloc(
            terminal: Terminal,
            allocator: *const c_void,
            out_ptr: *mut *mut u8,
            out_len: *mut usize,
        ) -> Result;
        pub fn ghostty_snapshot_decoder_new_buf(
            allocator: *const c_void,
            decoder: *mut SnapshotDecoder,
            ptr: *const u8,
            len: usize,
        ) -> Result;
        pub fn ghostty_snapshot_decoder_set(
            decoder: SnapshotDecoder,
            option: i32,
            value: *const c_void,
        ) -> Result;
        pub fn ghostty_snapshot_decoder_decode(
            decoder: SnapshotDecoder,
            terminal: *mut Terminal,
        ) -> Result;
        pub fn ghostty_snapshot_decoder_free(decoder: SnapshotDecoder);
        pub fn ghostty_free(allocator: *const c_void, ptr: *mut u8, len: usize);
    }
}

#[cfg(test)]
mod tests {
    use super::Terminal;

    fn restored(bytes: &[u8]) -> Terminal {
        let mut viewer = Terminal::new(80, 24).unwrap();
        viewer.write(bytes);
        viewer
    }

    #[test]
    fn repaint_reproduces_text_styles_and_hyperlinks() {
        let mut owner = Terminal::new(80, 24).unwrap();
        owner.write(b"plain \x1b[1;31mbold red\x1b[0m \x1b]8;;https://loopflow.studio\x1b\\docs\x1b]8;;\x1b\\\r\nsecond");
        let repaint = owner.repaint_screen().unwrap();
        assert!(
            repaint
                .windows(b"https://loopflow.studio".len())
                .any(|w| w == b"https://loopflow.studio"),
            "{}",
            String::from_utf8_lossy(&repaint)
        );
        let viewer = restored(&repaint);
        assert_eq!(viewer.text().unwrap(), owner.text().unwrap());
        assert_eq!(viewer.cursor(), owner.cursor());
    }

    #[test]
    fn repaint_restores_alternate_screen_and_input_modes() {
        let mut owner = Terminal::new(80, 24).unwrap();
        owner.write(b"\x1b[?1049h\x1b[?2004h\x1b[?1000hframe");
        assert!(owner.alternate_screen());
        let viewer = restored(&owner.repaint_screen().unwrap());
        assert!(viewer.alternate_screen());
        assert_eq!(viewer.text().unwrap().trim_end(), "frame");
        let repaint = String::from_utf8_lossy(&owner.repaint_screen().unwrap()).into_owned();
        assert!(repaint.contains("\x1b[?2004h"), "{repaint:?}");
        assert!(repaint.contains("\x1b[?1000h"), "{repaint:?}");
    }

    #[test]
    fn history_paints_the_newest_rows_oldest_first() {
        let mut owner = Terminal::new(80, 4).unwrap();
        for line in 0..10 {
            owner.write(format!("line {line}\r\n").as_bytes());
        }
        assert!(owner.scrollback_rows() >= 6, "{}", owner.scrollback_rows());
        let history = String::from_utf8_lossy(&owner.repaint_history(3).unwrap()).into_owned();
        let rows: Vec<&str> = history.split("\r\n").collect();
        assert_eq!(rows, ["line 4", "line 5", "line 6"], "{history:?}");
        assert!(owner.repaint_history(0).unwrap().is_empty());
    }

    #[test]
    fn snapshot_round_trips_the_screen() {
        let mut owner = Terminal::new(40, 6).unwrap();
        owner.write(b"\x1b[32mgreen\x1b[0m\r\nkept\x1b[31");
        let snapshot = owner.encode().unwrap();
        assert!(snapshot.starts_with(b"GHOSTSNP"));
        let decoded = Terminal::decode(&snapshot).unwrap();
        assert_eq!(decoded.text().unwrap(), owner.text().unwrap());
        assert_eq!(decoded.size(), (40, 6));
        assert!(!decoded.at_ground());
        assert_eq!(decoded.continuation().unwrap(), b"\x1b[31");
        assert!(Terminal::decode(b"not a snapshot").is_err());
    }

    #[test]
    fn answers_cursor_and_device_queries() {
        let mut owner = Terminal::new(80, 24).unwrap();
        owner.write(b"hello\x1b[6n\x1b[c");
        assert_eq!(owner.take_replies(), b"\x1b[1;6R\x1b[?62;22c");
        assert!(owner.take_replies().is_empty());
    }

    #[test]
    fn continuation_holds_a_split_sequence_until_it_completes() {
        let mut owner = Terminal::new(80, 24).unwrap();
        owner.write(b"text\x1b[?20");
        assert!(!owner.at_ground());
        assert_eq!(owner.continuation().unwrap(), b"\x1b[?20");
        owner.write(b"04h");
        assert!(owner.at_ground());
        assert!(owner.continuation().unwrap().is_empty());
    }
}
