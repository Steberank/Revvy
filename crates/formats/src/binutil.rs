//! Lectura little-endian de los binarios de Re-Volt.

use std::io::{self, Read};

use crate::FormatError;

pub struct Reader<R> {
    inner: R,
}

impl<R: Read> Reader<R> {
    pub fn new(inner: R) -> Self {
        Self { inner }
    }

    pub fn u8(&mut self) -> Result<u8, FormatError> {
        let mut buf = [0; 1];
        self.inner.read_exact(&mut buf)?;
        Ok(buf[0])
    }

    pub fn u16(&mut self) -> Result<u16, FormatError> {
        let mut buf = [0; 2];
        self.inner.read_exact(&mut buf)?;
        Ok(u16::from_le_bytes(buf))
    }

    pub fn i16(&mut self) -> Result<i16, FormatError> {
        Ok(self.u16()? as i16)
    }

    pub fn u32(&mut self) -> Result<u32, FormatError> {
        let mut buf = [0; 4];
        self.inner.read_exact(&mut buf)?;
        Ok(u32::from_le_bytes(buf))
    }

    pub fn i32(&mut self) -> Result<i32, FormatError> {
        Ok(self.u32()? as i32)
    }

    pub fn f32(&mut self) -> Result<f32, FormatError> {
        Ok(f32::from_le_bytes(self.u32()?.to_le_bytes()))
    }

    pub fn v3(&mut self) -> Result<[f32; 3], FormatError> {
        Ok([self.f32()?, self.f32()?, self.f32()?])
    }

    pub fn bytes(&mut self, n: usize) -> Result<Vec<u8>, FormatError> {
        let mut buf = vec![0; n];
        self.inner.read_exact(&mut buf)?;
        Ok(buf)
    }

    pub fn skip(&mut self, n: usize) -> Result<(), FormatError> {
        let mut left = n;
        let mut buf = [0; 256];
        while left > 0 {
            let n = left.min(buf.len());
            self.inner.read_exact(&mut buf[..n])?;
            left -= n;
        }
        Ok(())
    }

    pub fn rest(mut self) -> Result<Vec<u8>, FormatError> {
        let mut buf = Vec::new();
        self.inner.read_to_end(&mut buf)?;
        Ok(buf)
    }
}

impl From<io::Error> for FormatError {
    fn from(value: io::Error) -> Self {
        FormatError::Io(value)
    }
}
