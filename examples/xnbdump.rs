extern crate image;
extern crate squish;
extern crate xnb;

use image::{DynamicImage, ImageFormat, ImageBuffer};
use std::env;
use std::fs::File;
use std::io::BufReader;
use std::process;
use squish::{decompress_image, CompressType};
use xnb::{XNB, Asset, tide, Texture2d, SurfaceFormat};

fn usage() {
    println!("xnbdump [file.xnb]");
    err()
}

fn err() {
    process::exit(1);
}

fn main() {
    let mut args = env::args();
    let _self = args.next();
    let path = match args.next() {
        Some(path) => path,
        None => return usage(),
    };
    let f = match File::open(&path) {
        Ok(f) => f,
        Err(e) => {
            println!("Error opening file {}: {}", path, e);
            return err();
        }
    };
    let rdr = BufReader::new(f);
    let xnb = match XNB::from_buffer(rdr) {
        Ok(xnb) => xnb,
        Err(e) => {
            println!("Error parsing file contents: {:?}", e);
            return err();
        }
    };

    match xnb.primary {
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
            for ((g, c), m) in f.glyphs.into_iter().zip(f.cropping.into_iter()).zip(f.char_map.into_iter()) {
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
    }
}

fn dump_texture(texture: Texture2d) {
    for (i, data) in texture.mip_data.into_iter().enumerate() {
        let path = format!("data_{}.png", i);
        match File::create(&path) {
            Ok(mut f) => {
                let dynamic_image = {
                    let data = match texture.format {
                        SurfaceFormat::Color => data,
                        SurfaceFormat::Dxt3 => {
                            decompress_image(texture.width as i32,
                                             texture.height as i32,
                                             data.as_ptr() as *const _,
                                             CompressType::Dxt3)
                        }
                        f => panic!("can't handle surface format {:?}", f),
                    };

                    let img = ImageBuffer::from_raw(texture.width as u32,
                                                    texture.height as u32,
                                                    data).unwrap();
                    DynamicImage::ImageRgba8(img)
                };
                if let Err(e) = dynamic_image.save(&mut f, ImageFormat::PNG) {
                    println!("Error saving PNG: {}", e);
                    return err();
                }
            }

            Err(e) => {
                println!("Error creating file {}: {}", path, e);
                return err();
            }
        }
    }
}
