extern crate bitreader;
extern crate byteorder;

use byteorder::{ReadBytesExt, LittleEndian};
use std::collections::HashMap;
use std::io::{Read, Error as IoError, Cursor};

mod tide;
//mod lzx;

#[derive(Debug)]
struct TypeReader {
    name: String,
    _version: i32,
}

fn generic_types_from_reader(name: &str) -> Vec<&str> {
    let mut parts = name.split('`');
    let _main = parts.next().unwrap();
    let args = parts.next();
    if let Some(args) = args {
        let mut count = 0;
        let mut starts = vec![];
        let mut ends = vec![];
        let offset = 2;
        for (i, c) in args[offset..args.len()].chars().enumerate() {
            if c == '[' {
                if count == 0 {
                    starts.push(i + 1);
                }
                count += 1;
            }
            if c == ']' {
                count -= 1;
                if count == 0 {
                    ends.push(i);
                }
            }
        }
        assert_eq!(starts.len(), ends.len());
        starts.into_iter()
            .zip(ends.into_iter())
            .map(|(s, e)| &args[s+offset..e+offset])
            .map(|s| s.split(',').next().unwrap())
            .collect()
    } else {
        vec![]
    }
}

fn read_with_reader<R: Read>(name: &str, rdr: &mut R, readers: &[TypeReader]) -> Result<Asset, Error> {
    let main = name.split('`').next().unwrap().split(',').next().unwrap();
    let args = generic_types_from_reader(name);
    //println!("reading with {:?}", name);
    Ok(match main {
        "Microsoft.Xna.Framework.Content.Texture2DReader" =>
            Asset::Texture2d(try!(Texture2d::new(rdr))),
        "Microsoft.Xna.Framework.Content.DictionaryReader" => {
            //println!("{:?}", args);
            Asset::Dictionary(try!(Dictionary::new(args[0], args[1], rdr, readers)))
        }
        "Microsoft.Xna.Framework.Content.ArrayReader" => {
            //println!("{:?}", args);
            Asset::Array(try!(Array::new(args[0], rdr, readers)))
        }
        "Microsoft.Xna.Framework.Content.StringReader" =>
            Asset::String(try!(read_string(rdr))),
        "Microsoft.Xna.Framework.Content.Int32Reader" =>
            Asset::Int(try!(rdr.read_i32::<LittleEndian>())),
        "xTile.Pipeline.TideReader" => /*, xTile */
            Asset::Tide(try!(tide::read_tide(rdr))),
        _ => return Err(Error::UnknownReader(name.into())),
    })
}

#[derive(Debug)]
pub struct Array {
    pub vec: Vec<Asset>,
}

#[derive(Debug)]
pub struct Dictionary {
    pub map: HashMap<DictionaryKey, Asset>,
}

#[derive(PartialEq, Eq, Hash, Debug)]
pub enum DictionaryKey {
    Int(i32),
    String(String),
}

fn reader_from_type(typename: &str) -> Option<&'static str> {
    match typename {
        "System.Int32" => Some("Microsoft.Xna.Framework.Content.Int32Reader"),
        _ => None,
    }
}

fn read_dictionary_member<R: Read>(typename: &str, rdr: &mut R, readers: &[TypeReader])
                                   -> Result<Asset, Error> {
    if let Some(reader) = reader_from_type(typename) {
        read_with_reader(reader, rdr, readers)
    } else {
        read_object(rdr, readers)
    }
}

fn key_from_asset(asset: Asset) -> DictionaryKey {
    match asset {
        Asset::Int(i) => DictionaryKey::Int(i),
        Asset::String(s) => DictionaryKey::String(s),
        a => panic!("unsupported dictionary key {:?}", a)
    }
}

impl Dictionary {
    fn new<R: Read>(keytype: &str,
                    valtype: &str,
                    rdr: &mut R,
                    readers: &[TypeReader]) -> Result<Dictionary, Error> {
        let count = try!(rdr.read_u32::<LittleEndian>());
        let mut map = HashMap::new();
        for _ in 0..count {
            //println!("getting item {}/{}", i + 1, count);
            let key = key_from_asset(try!(read_dictionary_member(keytype, rdr, readers)));
            let value = try!(read_dictionary_member(valtype, rdr, readers));
            //println!("got {:?} => {:?}", key, value);
            map.insert(key, value);
        }
        Ok(Dictionary {
            map: map,
        })
    }
}

impl Array {
    fn new<R: Read>(typename: &str, rdr: &mut R, readers: &[TypeReader]) -> Result<Array, Error> {
        let count = try!(rdr.read_u32::<LittleEndian>());
        let mut vec = vec![];
        for _ in 0..count {
            let val = try!(read_dictionary_member(typename, rdr, readers));
            vec.push(val);
        }
        Ok(Array {
            vec: vec,
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

#[derive(Debug)]
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

#[derive(Debug)]
pub enum Asset {
    Null,
    Texture2d(Texture2d),
    Tide(Vec<u8>),
    Dictionary(Dictionary),
    Array(Array),
    String(String),
    Int(i32),
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
    UnexpectedObject(Asset),
}

impl From<IoError> for Error {
    fn from(e: IoError) -> Error {
        Error::Io(e)
    }
}

fn read_string<R: Read>(rdr: &mut R) -> Result<String, Error> {
    let len = try!(read_7bit_encoded_int(rdr));
    read_string_with_length(rdr, len)
}

fn read_string_with_length<R: Read>(rdr: &mut R, len: u32) -> Result<String, Error> {
    let mut s = String::new();
    for _ in 0..len {
        let val = try!(rdr.read_u8());
        s.push(val as char);
    }
    assert_eq!(s.len(), len as usize);
    Ok(s)
}

#[allow(dead_code)]
fn read_7bit_encoded_int<R: Read>(rdr: &mut R) -> Result<u32, Error> {
    let mut result = 0;
    let mut bits_read = 0;
    loop {
        let value = try!(rdr.read_u8());
        result |= ((value & 0x7F) as u32) << bits_read;
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
