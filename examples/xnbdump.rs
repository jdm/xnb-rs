extern crate xnb;

use std::env;
use std::fs::File;
use std::io::BufReader;
use std::process;
use xnb::XNB;

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
    let _xnb = match XNB::from_buffer(rdr) {
        Ok(xnb) => xnb,
        Err(e) => {
            println!("Error parsing file contents: {:?}", e);
            return err();
        }
    };
}
