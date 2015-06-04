// MSVC .lib specs:
// http://kishorekumar.net/pecoff_v8.1.htm

use std::collections::HashMap;
use std::env;
use std::error::Error;
use std::fs::File;
use std::io::prelude::*;
use std::io::{self, SeekFrom};
use std::mem;
use std::slice;
use std::str;

const MAG: [u8; 8] = *b"!<arch>\n";
const FMAG: [u8; 2] = *b"`\n";

#[repr(C)]
struct Header {
    name: [u8; 16],
    date: [u8; 12],
    uid: [u8; 6],
    gid: [u8; 6],
    mode: [u8; 8],
    size: [u8; 10],
    fmag: [u8; 2],
}

impl Header {
    fn as_mut_bytes(&mut self) -> &mut [u8] {
        unsafe {
            slice::from_raw_parts_mut(self as *mut _ as *mut u8,
                                      mem::size_of::<Self>())
        }
    }
}

fn main() {
    let mut args = env::args();
    doit(&args.nth(1).unwrap()).unwrap();
}

fn doit(file: &str) -> io::Result<()> {
    let mut f = try!(File::open(&file));
    let mut mag = [0; 8];
    try!(read_all(&mut f, &mut mag));
    if mag != MAG {
        return Err(bad(format!("invalid magic, found {:?} expected {:?}",
                               mag, MAG)));
    }

    let (off, header, symbol_table) = try!(next_file(&mut f)).unwrap();
    let symbol_table = if header.name == *b"/               " {
        println!(">> parsing symbol table");
        Some(try!(build_symbol_table(&symbol_table)))
    } else {
        try!(print(off, &header, &symbol_table, None, None));
        None
    };

    let (off, header, filename_table) = match try!(next_file(&mut f)) {
        Some(pair) => pair, None => return Ok(()),
    };
    let filename_table = if header.name == *b"//              " {
        println!(">> parsing filename table");
        Some(filename_table)
    } else {
        try!(print(off, &header, &filename_table, symbol_table.as_ref(), None));
        None
    };

    while let Some((off, header, contents)) = try!(next_file(&mut f)) {
        try!(print(off, &header, &contents, symbol_table.as_ref(),
                   filename_table.as_ref()));
    }
    Ok(())
}

fn print(offset: u32,
         header: &Header,
         contents: &[u8],
         symbol_table: Option<&HashMap<&str, Vec<u32>>>,
         filename_table: Option<&Vec<u8>>) -> io::Result<()> {
    println!("offset: {}", offset);
    if header.name[0] == b'/' && filename_table.is_some() {
        let offset = match str::from_utf8(&header.name[1..]).ok()
                              .and_then(|s| s.trim().parse().ok()) {
            Some(offset) => offset,
            None => return Err(bad(format!("invalid non-numeric filename"))),
        };
        let filename = &filename_table.unwrap()[offset..];
        let end = match filename.iter().position(|i| *i == b'\n') {
            Some(offset) => offset,
            None => return Err(bad(format!("filename table not terminated right"))),
        };
        print("name", &filename[..end]);
    } else {
        print("name", &header.name);
    }
    print("date", &header.date);
    print(" uid", &header.uid);
    print(" gid", &header.gid);
    print("mode", &header.mode);
    print("size", &header.size);

    if let Some(symbol_table) = symbol_table {
        println!("symbols within: ");
        for (k, v) in symbol_table.iter() {
            if v.contains(&offset) {
                println!("  {}", k);
            }
        }
    }

    print!("contents: ");
    match str::from_utf8(contents) {
        Ok(s) => println!("\n\t{}", s.replace("\n", "\n\t")),
        Err(..) if contents.len() < 100 => println!("{:?}", contents),
        Err(..) => println!("<binary>"),
    }
    println!("------------------------");
    return Ok(());

    fn print(field: &str, arr: &[u8]) {
        match str::from_utf8(arr) {
            Ok(s) => println!("{}: {:>15}", field, s.trim()),
            Err(..) => println!("{}: <bytes>{:?}", field, arr),
        }
    }
}

fn next_file(f: &mut File) -> io::Result<Option<(u32, Header, Vec<u8>)>> {
    let mut header: Header = unsafe { mem::zeroed() };
    let offset = try!(f.seek(SeekFrom::Current(0))) as u32;
    if !try!(read_all(f, header.as_mut_bytes())) {
        return Ok(None)
    }
    if header.fmag != FMAG {
        return Err(bad(format!("invalid file magic, found {:?} \
                                expected {:?}", header.fmag, FMAG)))
    }

    let n = match str::from_utf8(&header.size).ok()
                     .and_then(|s| s.trim().parse().ok()) {
        Some(n) => n,
        None => return Err(bad(format!("size field not a number: {:?}",
                                       header.size)))
    };

    let mut contents = Vec::new();
    try!(f.take(n).read_to_end(&mut contents));
    if contents.len() != n as usize {
        return Err(bad(format!("archive is truncated")))
    }

    if contents.len() % 2 == 1 {
        try!(f.seek(SeekFrom::Current(1)));
    }
    Ok(Some((offset as u32, header, contents)))
}

fn bad(s: String) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, s)
}

fn read_all(r: &mut Read, mut buf: &mut [u8]) -> io::Result<bool> {
    while buf.len() > 0 {
        match try!(r.read(buf)) {
            0 => return Ok(false),
            n => {
                let tmp = buf;
                buf = tmp.split_at_mut(n).1;
            }
        }
    }
    Ok(true)
}

fn build_symbol_table(mut contents: &[u8]) -> io::Result<HashMap<&str, Vec<u32>>> {
    let mut m = HashMap::new();
    let nsyms = try!(read_u32(&mut contents)) as usize;
    let (mut offsets, mut names) = contents.split_at(nsyms * 4);
    for _ in 0..nsyms {
        let nul_byte = match names.iter().position(|x| *x == 0) {
            Some(i) => i,
            None => return Err(bad(format!("invalid symbol table"))),
        };
        let name = match str::from_utf8(&names[..nul_byte]) {
            Ok(s) => s,
            Err(..) => return Err(bad(format!("symbol was not valid utf-8"))),
        };
        names = &names[nul_byte + 1..];
        let offset = try!(read_u32(&mut offsets));
        m.entry(name).or_insert(Vec::new()).push(offset);
    }
    Ok(m)
}

fn read_u32(arr: &mut &[u8]) -> io::Result<u32> {
    if arr.len() < 4 {
        return Err(bad(format!("symbol table needs to be at least 4 bytes")))
    }
    let ret = ((arr[0] as u32) << 24) |
              ((arr[1] as u32) << 16) |
              ((arr[2] as u32) <<  8) |
              ((arr[3] as u32) <<  0);
    *arr = &arr[4..];
    Ok(ret)
}
