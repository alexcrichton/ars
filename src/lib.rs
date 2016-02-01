use std::mem;
use std::slice;
use std::str::{self, FromStr};

pub const MAG: [u8; 8] = *b"!<arch>\n";
pub const FMAG: [u8; 2] = *b"`\n";

#[repr(C)]
pub struct Header {
    pub name: [u8; 16],
    pub date: [u8; 12],
    pub uid: [u8; 6],
    pub gid: [u8; 6],
    pub mode: [u8; 8],
    pub size: [u8; 10],
    pub fmag: [u8; 2],
}

impl Header {
    pub fn zero() -> Header {
        unsafe { mem::zeroed() }
    }

    pub fn as_bytes(&self) -> &[u8] {
        unsafe {
            slice::from_raw_parts(self as *const _ as *const u8,
                                  mem::size_of::<Self>())
        }
    }

    pub fn as_mut_bytes(&mut self) -> &mut [u8] {
        unsafe {
            slice::from_raw_parts_mut(self as *mut _ as *mut u8,
                                      mem::size_of::<Self>())
        }
    }

    pub fn name(&self) -> &[u8] { trim_spaces(&self.name) }

    pub fn date(&self) -> Option<u64> { parse(&self.name) }

    pub fn uid(&self) -> Option<u32> { parse(&self.uid) }

    pub fn gid(&self) -> Option<u32> { parse(&self.gid) }

    pub fn mode(&self) -> Option<u32> {
        str::from_utf8(trim_spaces(&self.mode)).ok().and_then(|s| {
            u32::from_str_radix(s, 8).ok()
        })
    }

    pub fn size(&self) -> Option<u64> { parse(&self.size) }

    pub fn valid(&self) -> bool { self.fmag == FMAG }
}

fn parse<T: FromStr>(b: &[u8]) -> Option<T> {
    str::from_utf8(trim_spaces(b)).ok().and_then(|s| s.parse().ok())
}

fn trim_spaces(b: &[u8]) -> &[u8] {
    match b.iter().position(|i| *i != b' ') {
        Some(i) => &b[..i],
        None => b,
    }
}
