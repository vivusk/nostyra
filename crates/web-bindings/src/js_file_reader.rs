//! wasm32 only: a registered content file read through its JS `read_at(offset, len)`
//! callback (a Worker's `FileReaderSync` over a `File` slice). Reads go through a
//! small read-ahead buffer: `read_to_end` and header probes issue many tiny reads,
//! and each callback costs a slice + a synchronous read on the JS side.

use std::cell::RefCell;
use std::io::{self, Read, Seek, SeekFrom};

use wasm_bindgen::JsValue;

/// Smallest read sent to JS (larger requests go through as they are).
const READ_AHEAD: u64 = 64 * 1024;

thread_local! {
    static READERS: RefCell<Vec<js_sys::Function>> = const { RefCell::new(Vec::new()) };
}

/// Keeps `read_at` and returns its number.
pub fn push_reader(read_at: js_sys::Function) -> usize {
    READERS.with(|readers| {
        let mut readers = readers.borrow_mut();
        readers.push(read_at);
        readers.len() - 1
    })
}

/// Releases every callback (open files then fail with "file reader released").
pub fn clear_readers() {
    READERS.with(|readers| readers.borrow_mut().clear());
}

/// An open JS-backed file: only numbers and bytes, so it is `Send` like every
/// content file.
pub struct JsFile {
    reader: usize,
    len: u64,
    pos: u64,
    /// Bytes read ahead, starting at `buf_start`.
    buf: Vec<u8>,
    buf_start: u64,
}

impl JsFile {
    pub fn new(reader: usize, len: u64) -> Self {
        Self {
            reader,
            len,
            pos: 0,
            buf: Vec::new(),
            buf_start: 0,
        }
    }

    /// Fills the buffer from `self.pos` with at least `want` bytes (file end permitting).
    fn fill(&mut self, want: u64) -> io::Result<()> {
        let size = want.max(READ_AHEAD).min(self.len - self.pos);
        let chunk = READERS.with(|readers| {
            let readers = readers.borrow();
            let read_at = readers
                .get(self.reader)
                .ok_or_else(|| io::Error::other("file reader released"))?;
            read_at
                .call2(
                    &JsValue::NULL,
                    &JsValue::from_f64(self.pos as f64),
                    &JsValue::from_f64(size as f64),
                )
                .map_err(|err| io::Error::other(format!("read_at failed: {err:?}")))
        })?;
        self.buf = js_sys::Uint8Array::new(&chunk).to_vec();
        self.buf_start = self.pos;
        Ok(())
    }
}

impl Read for JsFile {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let want = (buf.len() as u64).min(self.len.saturating_sub(self.pos));
        if want == 0 {
            return Ok(0);
        }
        let buffered =
            self.pos >= self.buf_start && self.pos < self.buf_start + self.buf.len() as u64;
        if !buffered {
            self.fill(want)?;
        }
        let offset = (self.pos - self.buf_start) as usize;
        let got = (self.buf.len() - offset).min(buf.len());
        buf[..got].copy_from_slice(&self.buf[offset..offset + got]);
        self.pos += got as u64;
        Ok(got)
    }
}

impl Seek for JsFile {
    fn seek(&mut self, to: SeekFrom) -> io::Result<u64> {
        let target = match to {
            SeekFrom::Start(pos) => Some(pos),
            SeekFrom::End(delta) => self.len.checked_add_signed(delta),
            SeekFrom::Current(delta) => self.pos.checked_add_signed(delta),
        };
        self.pos = target.ok_or_else(|| io::Error::other("seek before the start"))?;
        Ok(self.pos)
    }
}
