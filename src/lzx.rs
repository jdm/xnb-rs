use byteorder::{ReadBytesExt, BigEndian};
use std::io::{Write, Read, Error as IoError, Result as IoResult, Cursor, Seek, SeekFrom};

#[derive(Debug)]
pub enum Error {
    Io(IoError),
}

impl From<IoError> for Error {
    fn from(e: IoError) -> Error {
        Error::Io(e)
    }
}

pub fn decompress<R: Read>(mut rdr: R,
                           compressed_size: usize,
                           decompressed_size: usize) -> Result<Vec<u8>, Error> {
    let mut buffer = vec![0; decompressed_size];
    let mut frame_offset = 0;
    let mut pos = 0;
    let mut state = State::new();
    while pos < compressed_size {
        let (block_size, frame_size) = {
            let val = try!(rdr.read_u16::<BigEndian>());
            if val & 0xFF00 == 0xFF00 {
                let hi = (val & 0x00FF) << 8;
                let lo = try!(rdr.read_u8()) as u16;
                let frame_size = hi | lo;
                let block_size = try!(rdr.read_u16::<BigEndian>());
                pos += 4;
                (block_size, frame_size)
            } else {
                pos += 2;
                (val, 0x8000)
            }
        };

        println!("{} {}", block_size, frame_size);
        if block_size == 0 || frame_size == 0 {
            break;
        }

        let mut block = vec![0; block_size as usize];
        try!(rdr.read_exact(&mut block));
        let frame = &mut buffer[frame_offset..frame_size as usize];
        frame_offset += frame_size as usize;

        try!(decompress_block(&block, frame, &mut state));
        pos += block_size as usize;
        println!("{}", pos);
    }

    Ok(vec![])
}

struct State {
    intel_file_size: Option<u32>,
    block_remaining: usize,
    block_type: Option<BlockType>,
}

impl State {
    fn new() -> State {
        State {
            intel_file_size: None,
            block_remaining: 0,
            block_type: None,
        }
    }
}

enum BlockType {
    Verbatim,
    Aligned,
    Uncompressed,
}

struct BitReader<T: Read + Seek> {
    rdr: T,
    remaining: u8,
    bit_buffer: u32,
}

impl<T: Read + Seek> BitReader<T> {
    pub fn ensure(&mut self, bit_count: u8) -> Result<(), Error> {
        assert!(bit_count <= 32);
        while self.remaining < bit_count {
            let lo = try!(self.rdr.read_u8());
            let hi = try!(self.rdr.read_u8());
            self.bit_buffer |= (((hi as u32) << 8) | lo as u32) << (16 - self.remaining);
            self.remaining += 16;
        }
        Ok(())
    }

    pub fn peek(&self, bit_count: u8) -> u32 {
        assert!(bit_count <= self.remaining);
        self.bit_buffer >> (32 - bit_count)
    }

    pub fn remove(&mut self, bit_count: u8) {
        self.bit_buffer <<= bit_count;
        self.remaining -= bit_count;
    }

    pub fn read(&mut self, bit_count: u8) -> Result<u32, Error> {
        if bit_count > 0 {
            try!(self.ensure(bit_count));
            let bits = try!(self.peek(bit_count));
            self.remove(bit_count);
            bits
        } else {
            Ok(0)
        }
    }

    pub fn read_u8(&mut self) -> Result<u8, Error> {
        try!(self.rdr.read_u8())
    }

    pub fn read_u32(&mut self) -> Result<u32, Error> {
        let lo = try!(self.read_u8());
        let ml = try!(self.read_u8());
        let mh = try!(self.read_u8());
        let hi = try!(self.read_u8());
        Ok(lo | ml << 8 | mh << 16 | hi << 24)
    }
}

const MIN_MATCH: u16 = 2;
const MAX_MATCH: u16 = 257;
const NUM_CHARS: u16 = 256;

const PRETREE_NUM_ELEMENTS: u8 = 20;
const ALIGNED_NUM_ELEMENTS: u8 = 8;
const NUM_PRIMARY_LENGTHS: u8 = 7;
const NUM_SECONDARY_LENGTHS: u8 = 249;

const PRETREE_MAXSYMBOLS: u8 = PRETREE_NUM_ELEMENTS;
const PRETREE_TABLEBITS: u8 = 6;
const MAINTREE_MAXSYMBOLS: u16 = NUM_CHARS + 50 * 2;
const MAINTREE_TABLEBITS: u16 = 12;
const LENGTH_MAXSYMBOLS: u8 = NUM_SECONDARY_LENGTHS + 1;
const LENGTH_TABLEBITS: u8 = 12;
const ALIGNED_MAXSYMBOLS: u8 = ALIGNED_NUM_ELEMENTS;
const ALIGNED_TABLEBITS: u8 = 7;

const LENTABLE_SAFETY: u8 = 64;


fn decompress_block<W: Write>(buffer: &[u8], mut _wtr: W, state: &mut State) -> Result<(), Error> {
    let mut rdr = BitReader::new(buffer);
    let mut togo = buffer.len();
    let (mut r0, mut r1, mut r2) = (state.r0, state.r1, state.r2);

    if state.intel_file_size.is_none() {
        state.intel_file_size = Some(if try!(rdr.read(1)) != 0 {
            let hi = try!(rdr.read_u32(16));
            let lo = try!(rdr.read_u32(16));
            hi << 16 | lo
        } else {
            0
        });
    }

    while togo > 0 {
        if state.block_remaining == 0 {
            state.block_type = Some(match rdr.read(3) {
                1 => BlockType::Verbatim,
                2 => BlockType::Aligned,
                3 => BlockType::Uncompressed,
                _ => unreachable!(),
            });
            let hi = rdr.read(16) as usize;
            let lo = rdr.read(8) as usize;
            state.block_remaining = hi << 8 | lo;

            fn new_block_verbatim<T: Read + Seek>(rdr: &mut BitReader<T>, state: &mut State) {
                read_lengths(state.maintree_len, 0, 256,
                             state.pretree_maxsymbols, state.pretree_tablebits,
                             state.pretree_len, state.pretree_table, rdr);
                read_lengths(state.maintree_len, 256, state.main_elements,
                             state.pretree_maxsymbols, state.pretree_tablebits,
                             state.pretree_len, state.pretree_table, rdr);
                make_decode_table(MAINTREE_MAXSYMBOLS,
                                  MAINTREE_TABLEBITS,
                                  state.maintree_len,
                                  state.maintree_table);
                read_lengths(state.length_len, 0, NUM_SECONDARY_LENGTHS,
                             state.pretree_maxsymbols, state.pretree_tablebits,
                             state.pretree_len, state.pretree_table, rdr);

                make_decode_table(LENGTH_MAXSYMBOLS,
                                  LENGTH_TABLEBITS,
                                  state.legnth_len,
                                  state.length_table);
            }

            match state.block_type {
                BlockType::Aligned => {
                    for i in 0..8 {
                        let bits = rdr.read(3) as u8;
                        state.aligned_len[i] = bits;
                    }
                    make_decode_table(ALIGNED_MAXSYMBOLS,
                                      ALIGNED_TABLEBITS,
                                      state.aligned_len,
                                      state.aligned_table);
                    new_block_verbatim(state);
                }
                BlockType::Verbatim => {
                    new_block_verbatim(state)
                }
                BlockType::Uncompressed => {
                    rdr.ensure(16);
                    if rdr.remaining > 16 {
                        try!(rdr.rdr.seek(SeekFrom::Current(-2)));
                    }
                    r0 = try!(rdr.read_u32());
                    r1 = try!(rdr.read_u32());
                    r2 = try!(rdr.read_u32());
                }
            }
        }

        loop {
            let mut this_run = state.block_remaining;
            if this_run == 0 || togo == 0 {
                break;
            }

            if this_run > togo {
                this_run = togo;
            }
            togo -= this_run;
            state.block_remaining -= this_run;

            window_posn &= window_size - 1;

            assert!(window_posn + this_run <= window_size);

            match state.block_type {
                BlockType::Aligned => {
                    panic!()
                }
                BlockType::Verbatim => {
                    panic!()
                }
                BlockType::Uncompressed => {
                    window[window_posn..window_posn + this_run].copy_from_slice(buffer);
                }
            }
        }
    }

    Ok(())
}

fn read_lengths<T: Read + Seek>(lens: &mut [u8],
                                first: u32,
                                last: u32,
                                pretree_maxsymbols: &mut [u8],
                                pretree_tablebits: &mut [u8],
                                pretree_len: &mut [u8],
                                pretree_table: &mut [u8],
                                rdr: &mut BitReader<T>) -> Result<(), Error> {
    for x in 0..20 {
        let y = rdr.read(4) as u8;
        pretree_len[x] = y;
    }
    make_decode_table(pretree_maxsymbols,
                      pretree_tablebits,
                      pretree_len,
                      pretree_table);

    let mut x = first;
    while x < last {
        let z = read_huff_sym(pretree_table,
                              pretree_len,
                              PRETREE_MAXSYMBOLS,
                              PRETREE_TABLEBITS,
                              rdr);
        match z {
            17 => {
                let mut y = rdr.read(4) as u8 + 4;
                while y > 0 {
                    y -= 1;
                    if y == 0 {
                        break;
                    }
                    lens[x] = 0;
                    x += 1;
                }
            }
            18 => {
                let mut y = rdr.read(5) as u8 + 20;
                while y > 0 {
                    y -= 1;
                    if y == 0 {
                        break;
                    }
                    lens[x] = 0;
                    x += 1;
                }
            }
            19 => {
                let mut y = rdr.read(1) as u8 + 4;
                let z = read_huff_sym(pretree_table,
                                      pretree_len,
                                      PRETREE_MAXSYMBOLS,
                                      PRETREE_TABLEBITS,
                                      rdr);
                let z = lens[z] - z;
                if z < 0 {
                    z += 17;
                }
                while y > 0 {
                    y -= 1;
                    if y == 0 {
                        break;
                    }
                    lens[x] = z;
                    x += 1;
                }
            }
            _ => {
                let z = lens[x] - z;
                if z < 0 {
                    z += 17;
                }
                lens[x] = z;
                x += 1;
            }
        }
    }
}

fn read_huff_sym<T: Read + Seek>(table: &mut [u16],
                                 lengths: &mut [u8],
                                 nsyms: u32,
                                 nbits: u8,
                                 rdr: &mut BitReader<T>) -> Result<(), Error> {
    rdr.ensure(16);
    let mut i = table[rdr.peek(nbits)];
    if i >= nsyms {
        let mut j = 1 << (32 - nbits);
        loop {
            j >>= 1;
            i <<= 1;
            i |= if rdr.bit_buffer & j != 0 {
                1
            } else {
                0
            };
            if j == 0 {
                return 0;
            }
            i = table[i];
            if i < nsyms {
                break;
            }
        }
        j = lengths[i];
        rdr.remove(j);
        i
    }
}

fn make_decode_table(nsyms: u32, nbits: u8, length: &mut [u8], table: &mut [u16]) -> Result<(), ()> {
    let mut pos = 0;
    let mut table_mask = 1 << nbits;
    let mut bit_mask = table_mask >> 1;
    let mut next_symbol = bit_mask;
    let mut bit_num = 1;

    while bit_num <= nbits {
        for sym in 0..nsyms {
            if length[sym] != bit_num {
                continue;
            }
            let leaf = pos;
            pos += bit_mask;
            if pos > table_mask {
                return 1;
            }

            let mut fill = bit_mask;
            while fill > 0 {
                fill -= 1;
                table[leaf] = sym;
                leaf += 1;
            }
        }
        bit_mask >>= 1;
        bit_num += 1;
    }

    if pos != table_mask {
        for sym in pos..table_mask {
            table[sym] = 0;
        }

        pos <<= 16;
        table_mask << 16;
        bit_mask = 1 << 15;

        while bit_num <= 16 {
            for sym in 0..nsyms {
                if length[sym] == bit_num {
                    let mut leaf = pos >> 16;
                    for fill in 0..bit_num - nbits {
                        if table[leaf] == 0 {
                            table[next_symbol << 1] = 0;
                            table[(next_symbol << 1) + 1] = 0;
                            next_symbol += 1;
                            table[leaf] = next_symbol;
                        }
                        leaf = table[leaf] << 1;
                        if (pos >> (15 - fill) & 1) == 1 {
                            leaf += 1;
                        }
                    }
                    table[leaf] = sym;

                    pos += bit_mask;
                    if pos > table_mask {
                        return Err(());
                    }
                }
            }

            bit_mask >>= 1;
            bit_num += 1;
        }
    }

    if pos == table_mask {
        return Ok(());
    }

    for sym in 0..nsyms {
        if length[sym] != 0 {
            return Err(());
        }
    }

    Ok(())
}
