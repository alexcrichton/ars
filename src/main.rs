use std::env;
use std::error::Error;
use std::fs::File;
use std::io::prelude::*;
use std::io;
use std::mem;
use std::slice;
use std::str;

/*
#define ARMAG   "!<arch>\n" /* String that begins an archive file.  */
#define SARMAG  8       /* Size of that string.  */

#define ARFMAG  "`\n"       /* String in ar_fmag at end of each header.  */

__BEGIN_DECLS

struct ar_hdr
  {
    char ar_name[16];       /* Member file name, sometimes / terminated. */
    char ar_date[12];       /* File date, decimal seconds since Epoch.  */
    char ar_uid[6], ar_gid[6];  /* User and group IDs, in ASCII decimal.  */
    char ar_mode[8];        /* File mode, in ASCII octal.  */
    char ar_size[10];       /* File size, in ASCII decimal.  */
    char ar_fmag[2];        /* Always contains ARFMAG.  */
  };
*/

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
    let mut header: Header = unsafe { mem::zeroed() };
    loop {
        if !try!(read_all(&mut f, header.as_mut_bytes())) {
            break
        }
        if header.fmag != FMAG {
            return Err(bad(format!("invalid file magic, found {:?} \
                                    expected {:?}", header.fmag, FMAG)))
        }

        print("name", &header.name);
        print("date", &header.date);
        print("uid", &header.uid);
        print("gid", &header.gid);
        print("mode", &header.mode);
        print("size", &header.size);

        let s = match str::from_utf8(&header.size) {
            Ok(s) => s,
            Err(..) => return Err(bad(format!("size field is not utf-8: {:?}",
                                              header.size))),
        };
        let n = match s.trim().parse() {
            Ok(n) => n,
            Err(..) => return Err(bad(format!("size field not a number: {}", s)))
        };

        let mut contents = Vec::new();
        try!((&mut f).take(n).read_to_end(&mut contents));
        if contents.len() != n as usize {
            return Err(bad(format!("archive is truncated")))
        }

        println!("contents: ");
        match str::from_utf8(&contents) {
            Ok(s) => println!("\n\t{}", s.replace("\n", "\n\t")),
            Err(..) => println!("<binary>"),
        }
        println!("------------------------");
    }
    Ok(())
}

fn print(field: &str, arr: &[u8]) {
    match str::from_utf8(arr) {
        Ok(s) => println!("{}: {:>15}", field, s.trim()),
        Err(..) => println!("{}: <bytes>{:?}", field, arr),
    }
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
