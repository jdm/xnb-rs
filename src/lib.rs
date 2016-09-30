use std::io::{Read, Result as IoResult, Error as IoError};

pub struct XNB;

#[derive(Debug)]
pub enum Error {
    Void,
    Io(IoError),
}

impl From<IoError> for Error {
    fn from(e: IoError) -> Error {
        Error::Io(e)
    }
}

fn read_byte<R: Read>(rdr: &mut R) -> Result<u8, Error> {
    let mut byte = vec![0];
    try!(rdr.read_exact(&mut byte));
    Ok(byte[0])
}

fn read_u32<R: Read>(rdr: &mut R) -> Result<u32, Error> {
    let mut byte = vec![0, 0, 0, 0];
    try!(rdr.read_exact(&mut byte));
    Ok(byte[0] as u32 | (byte[1] as u32) << 8 | (byte[2] as u32) << 16 | (byte[3] as u32) << 24)
}

#[allow(dead_code)]
fn read_7bit_encoded_int<R: Read>(rdr: &mut R) -> Result<u8, Error> {
    let mut result = 0;
    let mut bits_read = 0;
    loop {
        let value = try!(read_byte(rdr));
        result |= (value & 0x7F) << bits_read;
        bits_read += 7;
        if value & 0x80 == 0 {
            return Ok(result);
        }
    }
}

struct StringReader<'a> {
    buffer: &'a [u8],
    pos: usize,
}

impl<'a> StringReader<'a> {
    fn new(buffer: &[u8]) -> StringReader {
        StringReader {
            buffer: buffer,
            pos: 0,
        }
    }
}

impl<'a> Read for StringReader<'a> {
    fn read(&mut self, buf: &mut [u8]) -> IoResult<usize> {
        let expected = buf.len();
        let actual = if self.buffer.len() - self.pos < expected {
            self.buffer.len() - self.pos
        } else {
            expected
        };
        buf.copy_from_slice(&self.buffer[self.pos..self.pos + actual]);
        self.pos += actual;
        Ok(actual)
    }
}

impl XNB {
    fn decompress<R: Read>(_rdr: R) -> Result<String, Error> {
        Ok(String::new())
    }

    fn from_uncompressed_buffer<R: Read>(_rdr: R) -> Result<XNB, Error> {
        Ok(XNB)
    }

    pub fn from_buffer<R: Read>(mut rdr: R) -> Result<XNB, Error> {
        let mut header = vec![0, 0, 0];
        try!(rdr.read_exact(&mut header));
        if header != b"XNB" {
            return Err(Error::Void);
        }
        let target = try!(read_byte(&mut rdr));
        if ['w', 'm', 'x'].iter().find(|&b| *b == target as char).is_none() {
            return Err(Error::Void);
        }

        let version = try!(read_byte(&mut rdr));
        if version != 5 {
            return Err(Error::Void);
        }

        let flag = try!(read_byte(&mut rdr));
        let is_compressed = flag & 0x80 != 0;

        let _compressed_size = try!(read_u32(&mut rdr));
        
        if is_compressed {
            let _decompressed_size = try!(read_u32(&mut rdr));
            let buffer = try!(XNB::decompress(rdr));
            XNB::from_uncompressed_buffer(StringReader::new(buffer.as_bytes()))
        } else {
            XNB::from_uncompressed_buffer(rdr)
        }
    }
}
