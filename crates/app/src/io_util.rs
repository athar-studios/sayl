use std::fs::File;
use std::io;

/// Writes `buf` into `file` at `offset` using positioned writes without mutating shared file offset.
pub fn write_at(file: &File, buf: &[u8], offset: u64) -> io::Result<()> {
    #[cfg(windows)]
    {
        use std::os::windows::fs::FileExt;
        let mut written = 0;
        while written < buf.len() {
            let n = file.seek_write(&buf[written..], offset + written as u64)?;
            if n == 0 {
                return Err(io::Error::new(
                    io::ErrorKind::WriteZero,
                    "failed to write whole buffer with seek_write",
                ));
            }
            written += n;
        }
        Ok(())
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::FileExt;
        file.write_all_at(buf, offset)
    }
    #[cfg(not(any(windows, unix)))]
    {
        compile_error!("unsupported platform for positioned writes");
    }
}
