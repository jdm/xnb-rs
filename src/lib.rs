extern crate bitreader;
extern crate byteorder;

use byteorder::{ReadBytesExt, LittleEndian};
use std::io::{Read, Error as IoError, Cursor};

//mod lzx;

pub struct XNB {
    buffer: Vec<u8>,
}

#[derive(Debug)]
pub enum Error {
    Void,
    Io(IoError),
    //Decompress(lzx::Error),
    CompressedXnb,
}

impl From<IoError> for Error {
    fn from(e: IoError) -> Error {
        Error::Io(e)
    }
}

#[allow(dead_code)]
fn read_7bit_encoded_int<R: Read>(rdr: &mut R) -> Result<u8, Error> {
    let mut result = 0;
    let mut bits_read = 0;
    loop {
        let value = try!(rdr.read_u8());
        result |= (value & 0x7F) << bits_read;
        bits_read += 7;
        if value & 0x80 == 0 {
            return Ok(result);
        }
    }
}

impl XNB {
    fn decompress<R: Read>(_rdr: R,
                           _compressed_size: usize,
                           _decompressed_size: usize) -> Result<Vec<u8>, Error> {
        //lzx::decompress(rdr, compressed_size, decompressed_size).map_err(|e| Error::Decompress(e))
        Err(Error::CompressedXnb)
    }

    fn from_uncompressed_buffer<R: Read>(mut rdr: R) -> Result<XNB, Error> {
        let mut buffer = vec![];
        try!(rdr.read_to_end(&mut buffer));
        Ok(XNB {
            buffer: buffer,
        })
    }

    pub fn from_buffer<R: Read>(mut rdr: R) -> Result<XNB, Error> {
        let mut header = vec![0, 0, 0];
        try!(rdr.read_exact(&mut header));
        if header != b"XNB" {
            return Err(Error::Void);
        }
        let target = try!(rdr.read_u8());
        if ['w', 'm', 'x'].iter().find(|&b| *b == target as char).is_none() {
            return Err(Error::Void);
        }

        let version = try!(rdr.read_u8());
        if version != 5 {
            return Err(Error::Void);
        }

        let flag = try!(rdr.read_u8());
        let is_compressed = flag & 0x80 != 0;

        let compressed_size = try!(rdr.read_u32::<LittleEndian>());
        
        if is_compressed {
            let decompressed_size = try!(rdr.read_u32::<LittleEndian>());
            let buffer = try!(XNB::decompress(rdr,
                                              compressed_size as usize - 14,
                                              decompressed_size as usize));
            XNB::from_uncompressed_buffer(Cursor::new(&buffer))
        } else {
            XNB::from_uncompressed_buffer(rdr)
        }
    }
}
