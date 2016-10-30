use byteorder::{LittleEndian, ReadBytesExt};
use {read_string_with_length, Error};
use std::io::{Read, Cursor};

struct TileSheet {
    id: String,
    description: String,
    image_source: String,
    sheet_size: (u32, u32),
    tile_size: (u32, u32),
    margin: (u32, u32),
    spacing: (u32, u32),
}

#[derive(Debug)]
enum PropertyValue {
    Bool(bool),
    Int(i32),
    Float(f32),
    String(String),
}

fn read_tide_string<R: Read>(rdr: &mut R) -> Result<String, Error> {
    let len = try!(rdr.read_u32::<LittleEndian>());
    read_string_with_length(rdr, len)
}

fn read_tide_properties<R: Read>(rdr: &mut R) -> Result<Vec<(String, PropertyValue)>, Error> {
    let num_properties = try!(rdr.read_u32::<LittleEndian>());

    let mut props = vec![];
    for _ in 0..num_properties {
        let name = try!(read_tide_string(rdr));

        let value = match try!(rdr.read_u8()) {
            0 => PropertyValue::Bool(try!(rdr.read_u8()) != 0),
            1 => PropertyValue::Int(try!(rdr.read_i32::<LittleEndian>())),
            2 => PropertyValue::Float(try!(rdr.read_f32::<LittleEndian>())),
            3 => PropertyValue::String(try!(read_tide_string(rdr))),
            _ => unreachable!("unexpected property type"),
        };
        props.push((name, value));
    }
    Ok(props)
}

fn read_static_tile<R: Read>(rdr: &mut R) -> Result<(), Error> {
    let _idx = try!(rdr.read_u32::<LittleEndian>());
    let _blend_mode = try!(rdr.read_u8());
    let properties = try!(read_tide_properties(rdr));
    print_properties(properties);
    Ok(())
}

fn print_properties(properties: Vec<(String, PropertyValue)>) {
    for (name, value) in properties {
        println!("{} = {:?}", name, value);
    }
}

pub fn read_tide<R: Read>(rdr: &mut R) -> Result<Vec<u8>, Error> {
    let size = try!(rdr.read_u32::<LittleEndian>());
    let mut buf = vec![0; size as usize];
    try!(rdr.read(&mut buf));

    {
        let mut rdr = Cursor::new(&buf);

        let mut header = vec![0; 6];
        try!(rdr.read(&mut header));
        if header != b"tBIN10" {
            return Err(Error::Void);
        }

        //let _ = try!(rdr.read_u32::<LittleEndian>());

        //let mut tilesheets = vec![];

        let map_id = try!(read_tide_string(&mut rdr));
        println!("{}", map_id);

        let map_description = try!(read_tide_string(&mut rdr));
        if !map_description.is_empty() {
            println!("{}", map_description);
        }

        let properties = try!(read_tide_properties(&mut rdr));
        print_properties(properties);

        let num_tilesheets = try!(rdr.read_u32::<LittleEndian>());

        for _ in 0..num_tilesheets {
            let tilesheet_name = try!(read_tide_string(&mut rdr));
            println!("{}", tilesheet_name);

            let description = try!(read_tide_string(&mut rdr));
            if !description.is_empty() {
                println!("{}", description);
            }

            let source = try!(read_tide_string(&mut rdr));
            println!("{}", source);

            let sheet_width = try!(rdr.read_u32::<LittleEndian>());
            let sheet_height = try!(rdr.read_u32::<LittleEndian>());
            println!("{}x{}", sheet_width, sheet_height);

            let tile_w = try!(rdr.read_u32::<LittleEndian>());
            let tile_h = try!(rdr.read_u32::<LittleEndian>());
            println!("{}x{}", tile_w, tile_h);

            let margin_w = try!(rdr.read_u32::<LittleEndian>());
            let margin_h = try!(rdr.read_u32::<LittleEndian>());
            println!("{}x{}", margin_w, margin_h);

            let spacing_w = try!(rdr.read_u32::<LittleEndian>());
            let spacing_h = try!(rdr.read_u32::<LittleEndian>());
            println!("{}x{}", spacing_w, spacing_h);

            let properties = try!(read_tide_properties(&mut rdr));
            print_properties(properties);
        }


        let num_layers = try!(rdr.read_u32::<LittleEndian>());

        for _ in 0..num_layers {
            println!("---");

            let layer_id = try!(read_tide_string(&mut rdr));
            println!("{}", layer_id);

            let visible = try!(rdr.read_u8()) != 0;
            println!("{}", if visible { "visible" } else { "invisible"});
            let description = try!(read_tide_string(&mut rdr));
            if !description.is_empty() {
                println!("{}", description);
            }
            let layer_w = try!(rdr.read_u32::<LittleEndian>());
            let layer_h = try!(rdr.read_u32::<LittleEndian>());
            println!("{}x{}", layer_w, layer_h);
            let tile_w = try!(rdr.read_u32::<LittleEndian>());
            let tile_h = try!(rdr.read_u32::<LittleEndian>());
            println!("{}x{}", tile_w, tile_h);

            let properties = try!(read_tide_properties(&mut rdr));
            print_properties(properties);

            let mut y = 0;
            while y < layer_h {
                let mut x = 0;
                while x < layer_w {
                    match try!(rdr.read_u8()) as char {
                        'T' => {
                            let _id = try!(read_tide_string(&mut rdr));
                        }
                        'S' => {
                            try!(read_static_tile(&mut rdr));
                            x += 1;
                        }
                        'N' => {
                            x += try!(rdr.read_u32::<LittleEndian>());
                        }
                        'A' => {
                            let _interval = try!(rdr.read_u32::<LittleEndian>());
                            let frame_count = try!(rdr.read_u32::<LittleEndian>());
                            let mut frame = 0;
                            while frame < frame_count {
                                match try!(rdr.read_u8()) as char {
                                    'T' => {
                                        let _id = try!(read_tide_string(&mut rdr));
                                    }
                                    'S' => {
                                        try!(read_static_tile(&mut rdr));
                                        frame += 1;
                                    }
                                    _ => unreachable!("unexpected animated frame type"),
                                }
                            }
                            let properties = try!(read_tide_properties(&mut rdr));
                            print_properties(properties);
                            x += 1;
                        }
                        _ => unreachable!("unexpected frame type"),
                    }
                }
                y += 1;
            }
        }
    }
    Ok(buf)
}
