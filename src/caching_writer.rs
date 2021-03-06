use anyhow::Result;
use log::*;
use std::io::Write;

/**
 * wrap a writer so that it is passthrough,
 * but also the written data is compressed and written into a buffer,
 * unless more than max_cache_size bytes is written, then the cache is dropped and it is pure passthrough.
 */
pub struct CachingWriter<W: Write> {
    max_cache_size: usize,
    zstd_writer: Option<zstd::stream::write::Encoder<Vec<u8>>>,
    out: W,
    bytes_written: u64,
}
impl<W: Write> CachingWriter<W> {
    pub fn new(out: W, max_cache_size: usize, compression_level: i32) -> Result<CachingWriter<W>> {
        Ok(CachingWriter {
            out,
            max_cache_size,
            zstd_writer: Some(zstd::stream::write::Encoder::new(
                Vec::new(),
                compression_level,
            )?),
            bytes_written: 0,
        })
    }
    pub fn finish(self) -> std::io::Result<(u64, Option<Vec<u8>>)> {
        if let Some(writer) = self.zstd_writer {
            let res = writer.finish()?;
            if res.len() <= self.max_cache_size {
                return Ok((self.bytes_written, Some(res)));
            }
        }
        Ok((self.bytes_written, None))
    }
}
impl<W: Write> Write for CachingWriter<W> {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        let written_bytes = match self.zstd_writer.as_mut() {
            Some(writer) => {
                let wrote = writer.write(buf)?;
                let compressed_len = writer.get_ref().len();
                trace!("wrote {} to zstd, len now {}", wrote, compressed_len);
                if compressed_len > self.max_cache_size {
                    debug!("cache longer than max, dropping");
                    //writer.finish();
                    self.zstd_writer.take().unwrap().finish()?;
                }
                self.out.write_all(&buf[0..wrote])?;
                Ok(wrote)
            }
            None => self.out.write(buf),
        }?;
        self.bytes_written += written_bytes as u64;
        Ok(written_bytes)
    }
    fn flush(&mut self) -> std::io::Result<()> {
        debug!("flushing");
        if let Some(writer) = self.zstd_writer.as_mut() {
            writer.flush()?;
        }
        self.out.flush()
    }
}
