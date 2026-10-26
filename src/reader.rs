//! Little-endian byte cursor over a chunk payload.

use std::fmt;

#[derive(Debug)]
pub struct Error(pub String);

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for Error {}

impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Self {
        Error(e.to_string())
    }
}

pub type Result<T> = std::result::Result<T, Error>;

macro_rules! bail {
    ($($arg:tt)*) => { return Err($crate::reader::Error(format!($($arg)*))) };
}
pub(crate) use bail;

pub struct Reader<'a> {
    data: &'a [u8],
    pos: usize,
}

impl<'a> Reader<'a> {
    pub fn new(data: &'a [u8]) -> Self {
        Reader { data, pos: 0 }
    }

    pub fn pos(&self) -> usize {
        self.pos
    }

    pub fn bytes(&mut self, n: usize) -> Result<&'a [u8]> {
        let Some(end) = self.pos.checked_add(n).filter(|&e| e <= self.data.len()) else {
            bail!("unexpected end of data: need {n} bytes at offset {}, have {}", self.pos, self.data.len() - self.pos);
        };
        let b = &self.data[self.pos..end];
        self.pos = end;
        Ok(b)
    }

    fn array<const N: usize>(&mut self) -> Result<[u8; N]> {
        Ok(self.bytes(N)?.try_into().unwrap())
    }

    pub fn u8(&mut self) -> Result<u8> {
        Ok(self.array::<1>()?[0])
    }

    pub fn u16(&mut self) -> Result<u16> {
        Ok(u16::from_le_bytes(self.array()?))
    }

    pub fn u32(&mut self) -> Result<u32> {
        Ok(u32::from_le_bytes(self.array()?))
    }

    pub fn i32(&mut self) -> Result<i32> {
        Ok(i32::from_le_bytes(self.array()?))
    }

    pub fn f32(&mut self) -> Result<f32> {
        Ok(f32::from_le_bytes(self.array()?))
    }

    pub fn vec2(&mut self) -> Result<[f32; 2]> {
        Ok([self.f32()?, self.f32()?])
    }

    pub fn vec3(&mut self) -> Result<[f32; 3]> {
        Ok([self.f32()?, self.f32()?, self.f32()?])
    }

    pub fn xform(&mut self) -> Result<[f32; 12]> {
        let mut m = [0.0; 12];
        for v in &mut m {
            *v = self.f32()?;
        }
        Ok(m)
    }

    /// .NET `BinaryWriter` string: LEB128 byte length, then UTF-8.
    pub fn str(&mut self) -> Result<String> {
        let mut len = 0usize;
        let mut shift = 0;
        loop {
            let b = self.u8()?;
            len |= ((b & 0x7f) as usize) << shift;
            if b & 0x80 == 0 {
                break;
            }
            shift += 7;
            if shift > 28 {
                bail!("string length varint too long at offset {}", self.pos);
            }
        }
        let bytes = self.bytes(len)?;
        String::from_utf8(bytes.to_vec()).map_err(|e| Error(format!("invalid UTF-8 string: {e}")))
    }

    /// `u32 count` followed by `count` items.
    pub fn list<T>(&mut self, mut item: impl FnMut(&mut Self) -> Result<T>) -> Result<Vec<T>> {
        let n = self.u32()? as usize;
        if n > self.data.len() - self.pos {
            bail!("list count {n} exceeds remaining data at offset {}", self.pos);
        }
        (0..n).map(|_| item(self)).collect()
    }

    /// Asserts that only zero padding (< 4 bytes) remains.
    pub fn finish(self) -> Result<()> {
        let rest = &self.data[self.pos..];
        if rest.len() >= 4 || rest.iter().any(|&b| b != 0) {
            bail!("{} unparsed bytes at offset {}: {:02x?}", rest.len(), self.pos, &rest[..rest.len().min(16)]);
        }
        Ok(())
    }
}
