//! A log file writer with a size cap: a very chatty core cannot fill the disk. When the
//! next line would pass the cap, one notice line is written and every later line is
//! dropped (no rotation within a run; the next session starts a new file).

use std::io::{self, Write};

/// Per-file cap (10 MiB).
pub const LOG_SIZE_CAP_BYTES: u64 = 10 * 1024 * 1024;
/// The last line of a capped log.
pub const LOG_TRUNCATED_NOTICE: &str = "log truncated (size limit)\n";

/// Wraps the log file; `tracing_subscriber` writes one formatted event per `write` call
/// (through a `Mutex`), so lines are never cut in half.
#[derive(Debug)]
pub struct SizeCappedWriter<W: Write> {
    inner: W,
    written: u64,
    cap: u64,
    truncated: bool,
}

impl<W: Write> SizeCappedWriter<W> {
    pub fn new(inner: W, cap: u64) -> Self {
        Self {
            inner,
            written: 0,
            cap,
            truncated: false,
        }
    }

    /// The wrapped writer (tests read back what was written).
    pub fn into_inner(self) -> W {
        self.inner
    }
}

impl<W: Write> Write for SizeCappedWriter<W> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        if self.truncated {
            return Ok(buf.len()); // dropped on purpose: callers must not see an error
        }
        let len = buf.len() as u64;
        if self.written.saturating_add(len) > self.cap {
            self.truncated = true;
            self.inner.write_all(LOG_TRUNCATED_NOTICE.as_bytes())?;
            return Ok(buf.len());
        }
        self.inner.write_all(buf)?;
        self.written += len;
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        self.inner.flush()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stops_at_the_cap_with_one_notice_line() {
        let mut writer = SizeCappedWriter::new(Vec::new(), 20);
        writer.write_all(b"0123456789\n").unwrap(); // 11 bytes
        writer.write_all(b"abcdefgh\n").unwrap(); // 20 bytes: exactly at the cap
        writer.write_all(b"too much\n").unwrap();
        writer.write_all(b"dropped\n").unwrap();
        let text = String::from_utf8(writer.into_inner()).unwrap();
        assert_eq!(
            text,
            format!("0123456789\nabcdefgh\n{LOG_TRUNCATED_NOTICE}")
        );
    }
}
