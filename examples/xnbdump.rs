extern crate image;
extern crate squish;
extern crate xnb;

use image::{DynamicImage, ImageBuffer};
use squish::{decompress_image, CompressType};
use std::env;
use std::fs::File;
use std::io::BufReader;
use std::process;
use xnb::{/*tide,*/ SurfaceFormat, Texture2d, XNB};

fn usage() {
    println!("xnbdump [file.xnb] [type]");
    err()
}

fn err() {
    process::exit(1);
}

trait Dumpable {
    fn dump(self);
}

impl Dumpable for xnb::Texture2d {
    fn dump(self) {
        dump_texture(self);
    }
}

impl Dumpable for xnb::SpriteFont {
    fn dump(self) {
        dump_texture(self.texture);
        println!("glyphs, cropping, char_map:");
        for ((g, c), m) in self
            .glyphs
            .into_iter()
            .zip(self.cropping.into_iter())
            .zip(self.char_map.into_iter())
        {
            println!("{:?} {:?} {}", g, c, m);
        }
        println!("v_space: {}", self.v_spacing);
        println!("h_space: {}", self.h_spacing);
        println!("kerning: {} elements", self.kerning.len());
        println!("default: {:?}", self.default);
    }
}

impl<T: std::fmt::Debug> Dumpable for Vec<T> {
    fn dump(self) {
        print!("[");
        for val in self {
            println!("{:?}, ", val);
        }
        print!("]");
    }
}

fn dump_xnb<T: xnb::Parse + Dumpable>(xnb: xnb::MaybeCompressedXNB) -> Result<(), xnb::Error> {
    let xnb: XNB<T> = match xnb {
        xnb::MaybeCompressedXNB::Uncompressed(xnb) => xnb.xnb()?,
        xnb::MaybeCompressedXNB::Compressed(xnb) => xnb.xnb(xnb::WindowSize::KB64)?,
    };
    xnb.primary.dump();
    Ok(())
}

fn main() {
    let mut args = env::args();
    let _self = args.next();
    let path = match args.next() {
        Some(path) => path,
        None => return usage(),
    };
    let typ = match args.next() {
        Some(typ) => typ,
        None => return usage(),
    };
    let f = match File::open(&path) {
        Ok(f) => f,
        Err(e) => {
            println!("Error opening file {}: {}", path, e);
            return err();
        }
    };
    let mut rdr = BufReader::new(f);
    let xnb = match xnb::MaybeCompressedXNB::from_buffer(&mut rdr) {
        Ok(xnb) => xnb,
        Err(e) => {
            println!("Error parsing file contents: {:?}", e);
            return err();
        }
    };

    let result = match &*typ {
        "texture2d" => dump_xnb::<xnb::Texture2d>(xnb),
        "stringarray" => dump_xnb::<Vec<String>>(xnb),
        "spritefont" => dump_xnb::<xnb::SpriteFont>(xnb),
        typ => unimplemented!("No support for \"{}\" XNBs", typ),
    };

    if let Err(e) = result {
        println!("Error dumping {}: {:?}", typ, e);
        return err();
    }

    /*match xnb.primary {
        Asset::Null => (),

        Asset::Texture2d(texture) => {
            dump_texture(texture);
        }

        Asset::Dictionary(dict) => {
            for (key, value) in dict.map {
                println!("{:?} => {:?}", key, value);
            }
        }

        Asset::Array(array) => {
            print!("[");
            for val in array.vec {
                println!("{:?}, ", val);
            }
            print!("]");
        }

        Asset::String(s) => {
            println!("{}", s);
        }

        Asset::Int(i) => {
            println!("{}", i);
        }

        Asset::Vector3(x, y, z) => {
            println!("({}, {}, {})", x, y, z);
        }

        Asset::Rectangle(r) => {
            println!("({}, {}) x ({}, {})", r.x, r.y, r.w, r.h);
        }

        Asset::Char(c) => {
            println!("{}", c);
        }

        Asset::Font(f) => {
            dump_texture(f.texture);
            println!("glyphs, cropping, char_map:");
            for ((g, c), m) in f
                .glyphs
                .into_iter()
                .zip(f.cropping.into_iter())
                .zip(f.char_map.into_iter())
            {
                println!("{:?} {:?} {}", g, c, m);
            }
            println!("v_space: {}", f.v_spacing);
            println!("h_space: {}", f.h_spacing);
            println!("kerning: {} elements", f.kerning.len());
            println!("default: {:?}", f.default);
        }

        Asset::Tide(map) => {
            if !map.properties.is_empty() {
                println!("Map properties:");
                tide::print_properties(&map.properties);
            }
            for ts in &map.tilesheets {
                if !ts.properties.is_empty() {
                    println!("Tilesheet {} properties:", ts.id);
                    tide::print_properties(&ts.properties);
                }
            }
            for layer in &map.layers {
                if !layer.properties.is_empty() {
                    println!("Layer {} properties:", layer.id);
                    tide::print_properties(&layer.properties);
                }
                for tile in &layer.tiles {
                    match *tile {
                        tide::Tile::Animated(ref tile) => {
                            for tile in &tile.frames {
                                if !tile.properties.is_empty() {
                                    println!("Tile {} properties:", tile.idx);
                                    tide::print_properties(&tile.properties);
                                }
                            }
                        }
                        tide::Tile::Static(ref tile) => {
                            if !tile.properties.is_empty() {
                                println!("Tile {} properties:", tile.idx);
                                tide::print_properties(&tile.properties);
                            }
                        }
                    }
                }
            }
        }
    }*/
}

fn dump_texture(texture: Texture2d) {
    for (i, data) in texture.mip_data.into_iter().enumerate() {
        let path = format!("data_{}.png", i);
        let dynamic_image = {
            let data = match texture.format {
                SurfaceFormat::Color => data,
                SurfaceFormat::Dxt3 => decompress_image(
                    texture.width as i32,
                    texture.height as i32,
                    data.as_ptr() as *const _,
                    CompressType::Dxt3,
                ),
                f => panic!("can't handle surface format {:?}", f),
            };

            let img =
                ImageBuffer::from_raw(texture.width as u32, texture.height as u32, data).unwrap();
            DynamicImage::ImageRgba8(img)
        };
        if let Err(e) = dynamic_image.save(path) {
            println!("Error saving PNG: {}", e);
            return err();
        }
    }
}
