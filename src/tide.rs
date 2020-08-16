use byteorder::{LittleEndian, ReadBytesExt};
use std::io::{Cursor, Read};
use {read_string_with_length, Error, Parse, TypeReader};

#[derive(Debug)]
pub struct TileSheet<T> {
    pub id: String,
    pub description: String,
    pub image_source: String,
    pub sheet_size: (u32, u32),
    pub tile_size: (u32, u32),
    pub margin: (u32, u32),
    pub spacing: (u32, u32),
    pub properties: T,
}

#[derive(Debug)]
pub enum PropertyValue {
    Bool(bool),
    Int(i32),
    Float(f32),
    String(String),
}

fn read_tide_string(rdr: &mut dyn Read) -> Result<String, Error> {
    let len = rdr.read_u32::<LittleEndian>()?;
    read_string_with_length(rdr, len)
}

fn read_tide_properties(rdr: &mut dyn Read) -> Result<Vec<(String, PropertyValue)>, Error> {
    let num_properties = rdr.read_u32::<LittleEndian>()?;

    let mut props = vec![];
    for _ in 0..num_properties {
        let name = read_tide_string(rdr)?;

        let value = match rdr.read_u8()? {
            0 => PropertyValue::Bool(rdr.read_u8()? != 0),
            1 => PropertyValue::Int(rdr.read_i32::<LittleEndian>()?),
            2 => PropertyValue::Float(rdr.read_f32::<LittleEndian>()?),
            3 => PropertyValue::String(read_tide_string(rdr)?),
            _ => unreachable!("unexpected property type"),
        };
        props.push((name, value));
    }
    Ok(props)
}

#[derive(Debug)]
pub struct StaticTile<T> {
    pub tilesheet: String,
    pub idx: u32,
    pub pos: (u32, u32),
    pub blend_mode: u8,
    pub properties: T,
}

fn read_static_tile<T: PropertyParse>(
    rdr: &mut dyn Read,
    tilesheet: String,
    pos: (u32, u32),
) -> Result<StaticTile<T>, Error> {
    let idx = rdr.read_u32::<LittleEndian>()?;
    let blend_mode = rdr.read_u8()?;
    let properties = T::parse(read_tide_properties(rdr)?);
    Ok(StaticTile {
        idx: idx,
        tilesheet: tilesheet,
        pos: pos,
        blend_mode: blend_mode,
        properties: properties,
    })
}

pub fn print_properties(properties: &[(String, PropertyValue)]) {
    for &(ref name, ref value) in properties {
        println!("{} = {:?}", name, value);
    }
}

pub trait PropertyParse {
    fn parse(props: Vec<(String, PropertyValue)>) -> Self;
}

#[derive(Debug)]
pub struct Map<T, U, V, W> {
    pub id: String,
    pub description: String,
    pub tilesheets: Vec<TileSheet<U>>,
    pub layers: Vec<Layer<V, W>>,
    pub properties: T,
}

impl<T, U, V, W> Map<T, U, V, W> {
    pub fn tilesheet(&self, sheet: &str) -> Option<&TileSheet<U>> {
        self.tilesheets.iter().find(|t| t.id == sheet)
    }
}

impl<T: PropertyParse, U: PropertyParse, V: PropertyParse, W: PropertyParse> Parse
    for Map<T, U, V, W>
{
    const READER: &'static str = "xTile.Pipeline.TideReader";
    fn try_parse(
        rdr: &mut dyn Read,
        _readers: &[TypeReader],
        _args: Vec<&str>,
    ) -> Result<Self, Error> {
        read_tide(rdr)
    }
}

#[derive(Debug)]
pub struct Layer<T, U> {
    pub id: String,
    pub description: String,
    pub tiles: Vec<Tile<U>>,
    pub visible: bool,
    pub size: (u32, u32),
    pub tile_size: (u32, u32),
    pub properties: T,
}

#[derive(Debug)]
pub enum Tile<T> {
    Static(StaticTile<T>),
    Animated(AnimatedTile<T>),
}

impl<T> Tile<T> {
    pub fn get_index(&self, tick: u32) -> u32 {
        match *self {
            Tile::Static(ref tile) => tile.idx,
            Tile::Animated(ref tile) => {
                tile.frames[(tick / tile.interval) as usize % tile.frames.len()].idx
            }
        }
    }

    pub fn get_tilesheet(&self) -> &str {
        match *self {
            Tile::Static(ref tile) => &tile.tilesheet,
            Tile::Animated(ref tile) => &tile.frames[0].tilesheet,
        }
    }

    pub fn get_pos(&self) -> (u32, u32) {
        match *self {
            Tile::Static(ref tile) => tile.pos,
            Tile::Animated(ref tile) => tile.frames[0].pos,
        }
    }

    pub fn properties(&self) -> &T {
        match *self {
            Tile::Static(ref tile) => &tile.properties,
            Tile::Animated(ref tile) => &tile.properties,
        }
    }
}

#[derive(Debug)]
pub struct AnimatedTile<T> {
    pub interval: u32,
    pub pos: (u32, u32),
    pub frames: Vec<StaticTile<T>>,
    pub properties: T,
}

pub fn read_tide<T, U, V, W>(rdr: &mut dyn Read) -> Result<Map<T, U, V, W>, Error>
where
    T: PropertyParse,
    U: PropertyParse,
    V: PropertyParse,
    W: PropertyParse,
{
    let size = rdr.read_u32::<LittleEndian>()?;
    let mut buf = vec![0; size as usize];
    rdr.read(&mut buf)?;

    let mut rdr = Cursor::new(&buf);

    let mut header = vec![0; 6];
    rdr.read(&mut header)?;
    if header != b"tBIN10" {
        return Err(Error::Void);
    }

    let map_id = read_tide_string(&mut rdr)?;
    println!("{}", map_id);

    let map_description = read_tide_string(&mut rdr)?;
    if !map_description.is_empty() {
        println!("{}", map_description);
    }

    let properties = T::parse(read_tide_properties(&mut rdr)?);

    let mut tilesheets = vec![];

    let num_tilesheets = rdr.read_u32::<LittleEndian>()?;
    for _ in 0..num_tilesheets {
        let tilesheet_name = read_tide_string(&mut rdr)?;
        println!("{}", tilesheet_name);

        let description = read_tide_string(&mut rdr)?;
        if !description.is_empty() {
            println!("{}", description);
        }

        let source = read_tide_string(&mut rdr)?;
        println!("{}", source);

        let sheet_width = rdr.read_u32::<LittleEndian>()?;
        let sheet_height = rdr.read_u32::<LittleEndian>()?;
        println!("{}x{}", sheet_width, sheet_height);

        let tile_w = rdr.read_u32::<LittleEndian>()?;
        let tile_h = rdr.read_u32::<LittleEndian>()?;
        println!("{}x{}", tile_w, tile_h);

        let margin_w = rdr.read_u32::<LittleEndian>()?;
        let margin_h = rdr.read_u32::<LittleEndian>()?;
        println!("{}x{}", margin_w, margin_h);

        let spacing_w = rdr.read_u32::<LittleEndian>()?;
        let spacing_h = rdr.read_u32::<LittleEndian>()?;
        println!("{}x{}", spacing_w, spacing_h);

        let properties = U::parse(read_tide_properties(&mut rdr)?);
        tilesheets.push(TileSheet {
            id: tilesheet_name,
            description: description,
            image_source: source,
            sheet_size: (sheet_width, sheet_height),
            tile_size: (tile_w, tile_h),
            margin: (margin_w, margin_h),
            spacing: (spacing_w, spacing_h),
            properties: properties,
        });
    }

    let mut layers = vec![];

    let num_layers = rdr.read_u32::<LittleEndian>()?;
    for _ in 0..num_layers {
        println!("---");

        let layer_id = read_tide_string(&mut rdr)?;
        println!("{}", layer_id);

        let visible = rdr.read_u8()? != 0;
        println!("{}", if visible { "visible" } else { "invisible" });
        let description = read_tide_string(&mut rdr)?;
        if !description.is_empty() {
            println!("{}", description);
        }
        let layer_w = rdr.read_u32::<LittleEndian>()?;
        let layer_h = rdr.read_u32::<LittleEndian>()?;
        println!("{}x{}", layer_w, layer_h);
        let tile_w = rdr.read_u32::<LittleEndian>()?;
        let tile_h = rdr.read_u32::<LittleEndian>()?;
        println!("{}x{}", tile_w, tile_h);

        let properties = V::parse(read_tide_properties(&mut rdr)?);

        let mut tiles = vec![];
        let mut tileset = None;

        let mut y = 0;
        while y < layer_h {
            let mut x = 0;
            while x < layer_w {
                match rdr.read_u8()? as char {
                    'T' => {
                        tileset = Some(read_tide_string(&mut rdr)?);
                    }
                    'S' => {
                        tiles.push(Tile::Static(read_static_tile(
                            &mut rdr,
                            tileset.clone().unwrap(),
                            (x, y),
                        )?));
                        x += 1;
                    }
                    'N' => {
                        x += rdr.read_u32::<LittleEndian>()?;
                    }
                    'A' => {
                        let interval = rdr.read_u32::<LittleEndian>()?;
                        let frame_count = rdr.read_u32::<LittleEndian>()?;
                        let mut frames = vec![];
                        let mut frame = 0;
                        while frame < frame_count {
                            match rdr.read_u8()? as char {
                                'T' => {
                                    tileset = Some(read_tide_string(&mut rdr)?);
                                }
                                'S' => {
                                    frames.push(read_static_tile(
                                        &mut rdr,
                                        tileset.clone().unwrap(),
                                        (x, y),
                                    )?);
                                    frame += 1;
                                }
                                _ => unreachable!("unexpected animated frame type"),
                            }
                        }
                        let properties = W::parse(read_tide_properties(&mut rdr)?);
                        tiles.push(Tile::Animated(AnimatedTile {
                            interval: interval,
                            frames: frames,
                            properties: properties,
                            pos: (x, y),
                        }));
                        x += 1;
                    }
                    _ => unreachable!("unexpected frame type"),
                }
            }
            y += 1;
        }

        layers.push(Layer {
            id: layer_id,
            description: description,
            visible: visible,
            size: (layer_w, layer_h),
            tile_size: (tile_w, tile_h),
            tiles: tiles,
            properties: properties,
        });
    }
    Ok(Map {
        id: map_id,
        description: map_description,
        tilesheets: tilesheets,
        layers: layers,
        properties: properties,
    })
}
