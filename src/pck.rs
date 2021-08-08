use crate::binnedwrite::Writeable;
use binrw::io::{Read, Seek};
use binrw::{derive_binread, BinRead, BinResult, PosValue, ReadOptions};
use std::io::Write;

pub const MAGIC: &[u8; 4] = b"GDPC";

#[derive_binread]
#[derive(Debug)]
#[br(magic = b"GDPC")]
pub struct Pck {
    #[br(assert(version == 1, "Version 1 only"))]
    pub version: u32,
    pub godot_version: GodotVersion,
    #[br(temp)]
    reserved: [u32; 16],
    #[br(temp)]
    entry_count: u32,
    #[br(count = entry_count)]
    pub entries: Vec<PckEntry>,
}

#[derive(BinRead, Debug)]
pub struct GodotVersion {
    pub major: u32,
    pub minor: u32,
    pub patch: u32,
}

impl Writeable for GodotVersion {
    type Args = ();

    fn write_to<W: Write>(&self, write: &mut W, _: Self::Args) -> std::io::Result<()> {
        self.major.write_to(write, ())?;
        self.minor.write_to(write, ())?;
        self.patch.write_to(write, ())?;

        Ok(())
    }
}

#[derive_binread]
#[derive(Debug)]
pub struct PckEntry {
    #[br(temp)]
    name_len: u32,
    #[br(args(name_len), parse_with = parse_utf8_string)]
    pub name: String,
    pub offset: PosValue<u64>,
    pub size: u64,
    pub md5: [u8; 16],
}

fn parse_utf8_string<R: Read + Seek>(
    reader: &mut R,
    _: &ReadOptions,
    (count,): (u32,),
) -> BinResult<String> {
    let mut bytes = Vec::with_capacity(count as usize);
    reader.take(count as u64).read_to_end(&mut bytes)?;
    // Godot encodes with some extra zero bytes sometimes. I'm not really sure why.
    Ok(String::from_utf8_lossy(&bytes)
        .trim_end_matches('\0')
        .to_owned())
}

impl PckEntry {
    pub fn set_offset_pos(&mut self, offset_base: u64) {
        self.offset.pos = offset_base + 4 + (self.name.len() as u64)
    }
}

impl Writeable for PckEntry {
    type Args = ();

    fn write_to<W: Write>(&self, write: &mut W, _: Self::Args) -> std::io::Result<()> {
        self.name.write_to(write, ())?;
        self.offset.val.write_to(write, ())?;
        self.size.write_to(write, ())?;
        write.write_all(&self.md5)?;

        Ok(())
    }
}
