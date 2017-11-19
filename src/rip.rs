use std::io::Write;

use byteorder::{BigEndian, WriteBytesExt};

fn sized_str_u16be(s: &str) -> Vec<u8> {
    let mut out = Vec::<u8>::new();
    out.write_u16::<BigEndian>(s.len() as u16).unwrap();
    out.write(s.as_bytes()).unwrap();
    out
}

pub fn write_rip<T: Write>(writer: &mut T, dfpwm: &Vec<u8>) {
    // "rip" magic
    writer.write_all(&[0x72, 0x69, 0x70]).unwrap();

    // track name
    writer.write_all(sized_str_u16be("").as_slice()).unwrap();

    // artist
    writer.write_all(sized_str_u16be("").as_slice()).unwrap();

    // album
    writer.write_all(sized_str_u16be("").as_slice()).unwrap();

    // DFPWM data length
    writer.write_u32::<BigEndian>(dfpwm.len() as u32).unwrap();
    writer.write_all(dfpwm.as_slice()).unwrap();
}