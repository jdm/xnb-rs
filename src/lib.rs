extern crate bitreader;
extern crate byteorder;

use byteorder::{LittleEndian, ReadBytesExt};
use std::collections::HashMap;
use std::hash::Hash;
use std::io::{Cursor, Error as IoError, Read};

pub mod tide;
//mod lzx;

#[derive(Debug)]
pub struct TypeReader {
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
        starts
            .into_iter()
            .zip(ends.into_iter())
            .map(|(s, e)| &args[s + offset..e + offset])
            .map(|s| s.split(',').next().unwrap())
            .collect()
    } else {
        vec![]
    }
}

pub trait Parse: Sized {
    const READER: &'static str;
    fn try_parse(
        _rdr: &mut dyn Read,
        _readers: &[TypeReader],
        _args: Vec<&str>,
    ) -> Result<Self, Error>;
    fn parse(
        name: &str,
        rdr: &mut dyn Read,
        readers: &[TypeReader],
        args: Vec<&str>,
    ) -> Result<Self, Error> {
        if name != Self::READER {
            return Err(Error::ReaderMismatch(
                name.to_string(),
                Self::READER.to_string(),
            ));
        }
        Self::try_parse(rdr, readers, args)
    }
}

impl Parse for Texture2d {
    const READER: &'static str = "Microsoft.Xna.Framework.Content.Texture2DReader";
    fn try_parse(
        rdr: &mut dyn Read,
        _readers: &[TypeReader],
        _args: Vec<&str>,
    ) -> Result<Self, Error> {
        Texture2d::new(rdr)
    }
}

impl<T: Parse> Parse for Vec<T> {
    //TODO: support list reader too: "Microsoft.Xna.Framework.Content.ListReader"
    const READER: &'static str = "Microsoft.Xna.Framework.Content.ArrayReader";
    fn try_parse(
        rdr: &mut dyn Read,
        readers: &[TypeReader],
        args: Vec<&str>,
    ) -> Result<Self, Error> {
        let count = rdr.read_u32::<LittleEndian>()?;
        let mut vec = vec![];
        for _ in 0..count {
            let val = read_dictionary_member(args[0], rdr, readers)?;
            vec.push(val);
        }
        Ok(vec)
    }
}

impl<K: Parse + Eq + Hash, V: Parse> Parse for Dictionary<K, V> {
    const READER: &'static str = "Microsoft.Xna.Framework.Content.DictionaryReader";
    fn try_parse(
        rdr: &mut dyn Read,
        readers: &[TypeReader],
        args: Vec<&str>,
    ) -> Result<Self, Error> {
        Dictionary::new(args[0], args[1], rdr, readers)
    }
}

impl Parse for Rectangle {
    const READER: &'static str = "Microsoft.Xna.Framework.Content.RectangleReader";
    fn try_parse(
        rdr: &mut dyn Read,
        _readers: &[TypeReader],
        _args: Vec<&str>,
    ) -> Result<Self, Error> {
        Rectangle::new(rdr)
    }
}

impl Parse for i32 {
    const READER: &'static str = "Microsoft.Xna.Framework.Content.Int32Reader";
    fn try_parse(
        rdr: &mut dyn Read,
        _readers: &[TypeReader],
        _args: Vec<&str>,
    ) -> Result<Self, Error> {
        rdr.read_i32::<LittleEndian>().map_err(Error::from)
    }
}

impl Parse for char {
    const READER: &'static str = "Microsoft.Xna.Framework.Content.CharReader";
    fn try_parse(
        rdr: &mut dyn Read,
        _readers: &[TypeReader],
        _args: Vec<&str>,
    ) -> Result<Self, Error> {
        rdr.read_u8().map(|b| b as char).map_err(Error::from)
    }
}

impl Parse for String {
    const READER: &'static str = "Microsoft.Xna.Framework.Content.StringReader";
    fn try_parse(
        rdr: &mut dyn Read,
        _readers: &[TypeReader],
        _args: Vec<&str>,
    ) -> Result<Self, Error> {
        read_string(rdr)
    }
}

impl Parse for SpriteFont {
    const READER: &'static str = "Microsoft.Xna.Framework.Content.SpriteFontReader";
    fn try_parse(
        rdr: &mut dyn Read,
        readers: &[TypeReader],
        _args: Vec<&str>,
    ) -> Result<Self, Error> {
        SpriteFont::new(rdr, readers)
    }
}

impl Parse for Vector3 {
    const READER: &'static str = "Microsoft.Xna.Framework.Content.Vector3Reader";
    fn try_parse(
        rdr: &mut dyn Read,
        _readers: &[TypeReader],
        _args: Vec<&str>,
    ) -> Result<Self, Error> {
        Ok(Vector3(
            rdr.read_f32::<LittleEndian>()?,
            rdr.read_f32::<LittleEndian>()?,
            rdr.read_f32::<LittleEndian>()?,
        ))
    }
}

fn read_with_reader<T: Parse>(
    name: &str,
    rdr: &mut dyn Read,
    readers: &[TypeReader],
) -> Result<T, Error> {
    let main = name.split('`').next().unwrap().split(',').next().unwrap();
    let args = generic_types_from_reader(name);
    //println!("reading with {:?}", name);
    T::parse(main, rdr, readers, args)
}

#[derive(Debug)]
pub struct Array<T> {
    pub vec: Vec<T>,
}

#[derive(Debug)]
pub struct Dictionary<K: Eq + Hash, V> {
    pub map: HashMap<K, V>,
}

#[derive(PartialEq, Eq, Hash, Debug)]
pub enum DictionaryKey {
    Int(i32),
    String(String),
}

fn reader_from_type(typename: &str) -> Option<&'static str> {
    match typename {
        "System.Int32" => Some("Microsoft.Xna.Framework.Content.Int32Reader"),
        "System.Char" => Some("Microsoft.Xna.Framework.Content.CharReader"),
        "Microsoft.Xna.Framework.Vector3" => Some("Microsoft.Xna.Framework.Content.Vector3Reader"),
        "Microsoft.Xna.Framework.Rectangle" => {
            Some("Microsoft.Xna.Framework.Content.RectangleReader")
        }
        _ => None,
    }
}

fn read_dictionary_member<T: Parse>(
    typename: &str,
    rdr: &mut dyn Read,
    readers: &[TypeReader],
) -> Result<T, Error> {
    //println!("checking {}" ,typename);
    if let Some(reader) = reader_from_type(typename) {
        read_with_reader(reader, rdr, readers)
    } else {
        read_object(rdr, readers)
    }
}

impl<K: Parse + Eq + Hash, V: Parse> Dictionary<K, V> {
    fn new(
        keytype: &str,
        valtype: &str,
        rdr: &mut dyn Read,
        readers: &[TypeReader],
    ) -> Result<Dictionary<K, V>, Error> {
        let count = rdr.read_u32::<LittleEndian>()?;
        let mut map = HashMap::new();
        for _ in 0..count {
            //println!("getting item {}/{}", i + 1, count);
            let key = read_dictionary_member(keytype, rdr, readers)?;
            let value = read_dictionary_member(valtype, rdr, readers)?;
            //println!("got {:?} => {:?}", key, value);
            map.insert(key, value);
        }
        Ok(Dictionary { map: map })
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
    fn new(rdr: &mut dyn Read) -> Result<Texture2d, Error> {
        let format = SurfaceFormat::from(rdr.read_u32::<LittleEndian>()?)?;
        let w = rdr.read_u32::<LittleEndian>()? as usize;
        let h = rdr.read_u32::<LittleEndian>()? as usize;
        let mip_count = rdr.read_u32::<LittleEndian>()?;
        let mut mip_data = vec![];
        for _ in 0..mip_count {
            let data_size = rdr.read_u32::<LittleEndian>()? as usize;
            let mut data = vec![0; data_size];
            rdr.read(&mut data)?;
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
pub struct SpriteFont {
    pub texture: Texture2d,
    pub glyphs: Vec<Rectangle>,
    pub cropping: Vec<Rectangle>,
    pub char_map: Vec<char>,
    pub v_spacing: i32,
    pub h_spacing: f32,
    pub kerning: Vec<Vector3>,
    pub default: Option<char>,
}

impl SpriteFont {
    fn new(rdr: &mut dyn Read, readers: &[TypeReader]) -> Result<SpriteFont, Error> {
        let texture = read_object::<Texture2d>(rdr, readers)?;
        let glyphs = read_object::<Vec<Rectangle>>(rdr, readers)?;
        let cropping = read_object::<Vec<Rectangle>>(rdr, readers)?;
        let char_map = read_object::<Vec<char>>(rdr, readers)?;
        let v_spacing = rdr.read_i32::<LittleEndian>()?;
        let h_spacing = rdr.read_f32::<LittleEndian>()?;
        let kerning = read_object::<Vec<Vector3>>(rdr, readers)?;
        //XXXjdm should be full UTF-8 char read
        let default = read_nullable::<char, _>(rdr, |rdr| {
            rdr.read_u8().map(|b| b as char).map_err(Error::Io)
        })?;
        Ok(SpriteFont {
            texture: texture,
            glyphs: glyphs,
            cropping: cropping,
            char_map: char_map,
            v_spacing: v_spacing,
            h_spacing: h_spacing,
            kerning: kerning,
            default: default,
        })
    }
}

#[derive(Debug)]
pub struct Rectangle {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
}

impl Rectangle {
    fn new(rdr: &mut dyn Read) -> Result<Rectangle, Error> {
        Ok(Rectangle {
            x: rdr.read_i32::<LittleEndian>()?,
            y: rdr.read_i32::<LittleEndian>()?,
            w: rdr.read_i32::<LittleEndian>()?,
            h: rdr.read_i32::<LittleEndian>()?,
        })
    }
}

#[derive(Debug)]
pub struct Vector3(f32, f32, f32);

pub struct XNB<T> {
    pub primary: T,
}

impl<T: Parse> XNB<T> {
    fn new(buffer: Vec<u8>) -> Result<XNB<T>, Error> {
        let mut rdr = Cursor::new(&buffer);
        let num_readers = read_7bit_encoded_int(&mut rdr)?;
        let mut readers = vec![];
        for _ in 0..num_readers {
            readers.push(TypeReader {
                name: read_string(&mut rdr)?,
                _version: rdr.read_i32::<LittleEndian>()?,
            });
            //println!("reader: {}", readers.last().unwrap().name);
        }
        let num_shared = read_7bit_encoded_int(&mut rdr)?;
        assert_eq!(num_shared, 0);
        let asset = read_object(&mut rdr, &readers)?;
        Ok(XNB { primary: asset })
    }
}

fn read_object<T: Parse>(rdr: &mut dyn Read, readers: &[TypeReader]) -> Result<T, Error> {
    let id = read_7bit_encoded_int(rdr)? as usize;
    assert!(id != 0);
    read_with_reader(&readers[id - 1].name, rdr, readers)
}

fn read_nullable<T: Parse, F: Fn(&mut dyn Read) -> Result<T, Error>>(
    rdr: &mut dyn Read,
    value: F,
) -> Result<Option<T>, Error> {
    let has_value = rdr.read_u8()? == 1;
    if !has_value {
        return Ok(None);
    }
    value(rdr).map(Option::Some)
}

#[derive(Debug)]
pub enum Error {
    Void,
    Io(IoError),
    //Decompress(lzx::Error),
    CompressedXnb,
    UnknownReader(String),
    UnrecognizedSurfaceFormat(u32),
    ReaderMismatch(String, String),
}

impl From<IoError> for Error {
    fn from(e: IoError) -> Error {
        Error::Io(e)
    }
}

fn read_string(rdr: &mut dyn Read) -> Result<String, Error> {
    let len = read_7bit_encoded_int(rdr)?;
    read_string_with_length(rdr, len)
}

fn read_string_with_length(rdr: &mut dyn Read, len: u32) -> Result<String, Error> {
    let mut s = String::new();
    for _ in 0..len {
        let val = rdr.read_u8()?;
        s.push(val as char);
    }
    assert_eq!(s.len(), len as usize);
    Ok(s)
}

#[allow(dead_code)]
fn read_7bit_encoded_int(rdr: &mut dyn Read) -> Result<u32, Error> {
    let mut result = 0;
    let mut bits_read = 0;
    loop {
        let value = rdr.read_u8()?;
        result |= ((value & 0x7F) as u32) << bits_read;
        bits_read += 7;
        if value & 0x80 == 0 {
            return Ok(result);
        }
    }
}

impl<T: Parse> XNB<T> {
    fn decompress(
        _rdr: &dyn Read,
        _compressed_size: usize,
        _decompressed_size: usize,
    ) -> Result<Vec<u8>, Error> {
        //lzx::decompress(rdr, compressed_size, decompressed_size).map_err(|e| Error::Decompress(e))
        Err(Error::CompressedXnb)
    }

    fn from_uncompressed_buffer(rdr: &mut dyn Read) -> Result<XNB<T>, Error> {
        let mut buffer = vec![];
        rdr.read_to_end(&mut buffer)?;
        XNB::new(buffer)
    }

    pub fn from_buffer(rdr: &mut dyn Read) -> Result<XNB<T>, Error> {
        let mut header = vec![0, 0, 0];
        rdr.read_exact(&mut header)?;
        if header != b"XNB" {
            return Err(Error::Void);
        }
        let target = rdr.read_u8()?;
        if ['w', 'm', 'x']
            .iter()
            .find(|&b| *b == target as char)
            .is_none()
        {
            return Err(Error::Void);
        }

        let version = rdr.read_u8()?;
        if version != 5 {
            return Err(Error::Void);
        }

        let flag = rdr.read_u8()?;
        let is_compressed = flag & 0x80 != 0;

        let compressed_size = rdr.read_u32::<LittleEndian>()?;

        if is_compressed {
            let decompressed_size = rdr.read_u32::<LittleEndian>()?;
            let buffer = Self::decompress(
                rdr,
                compressed_size as usize - 14,
                decompressed_size as usize,
            )?;
            XNB::from_uncompressed_buffer(&mut Cursor::new(&buffer))
        } else {
            XNB::from_uncompressed_buffer(rdr)
        }
    }
}
