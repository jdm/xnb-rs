extern crate image;
extern crate xnb;

use image::{DynamicImage, ImageFormat, RgbaImage};
use std::env;
use std::fs::File;
use std::io::BufReader;
use std::process;
use xnb::{XNB, Asset, tide};

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
            for (i, data) in texture.mip_data.into_iter().enumerate() {
                let path = format!("data_{}.png", i);
                match File::create(&path) {
                    Ok(mut f) => {
                        let img = RgbaImage::from_raw(texture.width as u32,
                                                     texture.height as u32,
                                                     data).unwrap();
                        let dynamic_image = DynamicImage::ImageRgba8(img);
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

        Asset::Tide(map) => {
            tide::print_properties(&map.properties);
            for ts in &map.tilesheets {
                tide::print_properties(&ts.properties);
            }
            for layer in &map.layers {
                tide::print_properties(&layer.properties);
                for tile in &layer.tiles {
                    match *tile {
                        tide::Tile::Animated(ref tile) => {
                            for tile in &tile.frames {
                                tide::print_properties(&tile.properties);
                            }
                        }
                        tide::Tile::Static(ref tile) => {
                            tide::print_properties(&tile.properties);
                        }
                    }
                }
            }
        }
    }
}
