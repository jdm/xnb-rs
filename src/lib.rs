extern crate bitreader;
extern crate byteorder;

use byteorder::{ReadBytesExt, LittleEndian};
use std::collections::HashMap;
use std::io::{Read, Error as IoError, Cursor};

//mod lzx;

struct TypeReader {
    name: String,
    _version: i32,
}

fn read_with_reader<R: Read>(name: &str, rdr: &mut R, readers: &[TypeReader]) -> Result<Asset, Error> {
    Ok(match name.split(',').next().unwrap() {
        "Microsoft.Xna.Framework.Content.Texture2DReader" =>
            Asset::Texture2d(try!(Texture2d::new(rdr))),
        "Microsoft.Xna.Framework.Content.DictionaryReader`2[[System.String" =>
            Asset::DictionaryString(try!(DictionaryString::new(rdr, readers))),
        "Microsoft.Xna.Framework.Content.StringReader" =>
            Asset::String(try!(read_string(rdr))),
        s => return Err(Error::UnknownReader(s.into())),
    })
}

pub struct DictionaryString {
    pub map: HashMap<String, String>,
}

impl DictionaryString {
    fn new<R: Read>(rdr: &mut R, readers: &[TypeReader]) -> Result<DictionaryString, Error> {
        let count = try!(rdr.read_u32::<LittleEndian>());
        let mut map = HashMap::new();
        for _ in 0..count {
            let key = match try!(read_object(rdr, readers)) {
                Asset::String(s) => s,
                _ => return Err(Error::UnexpectedObject),
            };
            let value = match try!(read_object(rdr, readers)) {
                Asset::String(s) => s,
                _ => return Err(Error::UnexpectedObject),
            };
            map.insert(key, value);
        }
        Ok(DictionaryString {
            map: map,
        })
    }
}

#[derive(Debug)]
pub enum SurfaceFormat {
    Color,
    Bgr565,
    Bgra5551,
    Bgra4444,
    Dxt1,
    Dxt3,
    Dxt5,
    NormalizedByte2,
    NormalizedByte4,
    Rgba1010102,
    Rg32,
    Rgba64,
    Alpha8,
    Single,
    Vector2,
    Vector4,
    HalfSingle,
    HalfVector2,
    HalfVector4,
    HdrBlendable,
}

impl SurfaceFormat {
    fn from(val: u32) -> Result<SurfaceFormat, Error> {
        Ok(match val {
            0 => SurfaceFormat::Color,
            1 => SurfaceFormat::Bgr565,
            2 => SurfaceFormat::Bgra5551,
            3 => SurfaceFormat::Bgra4444,
            4 => SurfaceFormat::Dxt1,
            5 => SurfaceFormat::Dxt3,
            6 => SurfaceFormat::Dxt5,
            7 => SurfaceFormat::NormalizedByte2,
            8 => SurfaceFormat::NormalizedByte4,
            9 => SurfaceFormat::Rgba1010102,
            10 => SurfaceFormat::Rg32,
            11 => SurfaceFormat::Rgba64,
            12 => SurfaceFormat::Alpha8,
            13 => SurfaceFormat::Single,
            14 => SurfaceFormat::Vector2,
            15 => SurfaceFormat::Vector4,
            16 => SurfaceFormat::HalfSingle,
            17 => SurfaceFormat::HalfVector2,
            18 => SurfaceFormat::HalfVector4,
            19 => SurfaceFormat::HdrBlendable,
            f => return Err(Error::UnrecognizedSurfaceFormat(f)),
        })
    }
}

pub struct Texture2d {
    pub format: SurfaceFormat,
    pub width: usize,
    pub height: usize,
    pub mip_data: Vec<Vec<u8>>,
}

impl Texture2d {
    fn new<R: Read>(rdr: &mut R) -> Result<Texture2d, Error> {
        let format = try!(SurfaceFormat::from(try!(rdr.read_u32::<LittleEndian>())));
        let w = try!(rdr.read_u32::<LittleEndian>()) as usize;
        let h = try!(rdr.read_u32::<LittleEndian>()) as usize;
        let mip_count = try!(rdr.read_u32::<LittleEndian>());
        let mut mip_data = vec![];
        for _ in 0..mip_count {
            let data_size = try!(rdr.read_u32::<LittleEndian>()) as usize;
            let mut data = vec![0; data_size];
            try!(rdr.read(&mut data));
            mip_data.push(data);
        }
        Ok(Texture2d {
            format: format,
            width: w,
            height: h,
            mip_data: mip_data,
        })
    }
}

pub enum Asset {
    Null,
    Texture2d(Texture2d),
    DictionaryString(DictionaryString),
    String(String),
}

pub struct XNB {
    pub primary: Asset,
}

impl XNB {
    fn new(buffer: Vec<u8>) -> Result<XNB, Error> {
        let mut rdr = Cursor::new(&buffer);
        let num_readers = try!(read_7bit_encoded_int(&mut rdr));
        let mut readers = vec![];
        for _ in 0..num_readers {
            readers.push(TypeReader {
                name: try!(read_string(&mut rdr)),
                _version: try!(rdr.read_i32::<LittleEndian>()),
            });
        }
        let num_shared = try!(read_7bit_encoded_int(&mut rdr));
        assert_eq!(num_shared, 0);
        let asset = try!(read_object(&mut rdr, &readers));
        Ok(XNB {
            primary: asset,
        })
    }
}

fn read_object<R: Read>(rdr: &mut R, readers: &[TypeReader]) -> Result<Asset, Error> {
    let id = try!(read_7bit_encoded_int(rdr)) as usize;
    if id == 0 {
        return Ok(Asset::Null);
    }
    read_with_reader(&readers[id - 1].name, rdr, readers)
}

#[derive(Debug)]
pub enum Error {
    Void,
    Io(IoError),
    //Decompress(lzx::Error),
    CompressedXnb,
    UnknownReader(String),
    UnrecognizedSurfaceFormat(u32),
    UnexpectedObject,
}

impl From<IoError> for Error {
    fn from(e: IoError) -> Error {
        Error::Io(e)
    }
}

fn read_string<R: Read>(rdr: &mut R) -> Result<String, Error> {
    let len = try!(read_7bit_encoded_int(rdr));
    let mut s = String::new();
    for _ in 0..len {
        let val = try!(rdr.read_u8());
        s.push(val as char);
    }
    Ok(s)
}

#[allow(dead_code)]
fn read_7bit_encoded_int<R: Read>(rdr: &mut R) -> Result<u32, Error> {
    let mut result = 0;
    let mut bits_read = 0;
    loop {
        let value = try!(rdr.read_u8());
        result |= ((value & 0x7F) << bits_read) as u32;
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
        XNB::new(buffer)
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
