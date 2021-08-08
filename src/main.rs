//#![deny(warnings)]

use std::fs::OpenOptions;
use std::path::PathBuf;

use binrw::io::{Cursor, Write};
use binrw::{BinReaderExt, PosValue};
use color_eyre::eyre::WrapErr;
use color_eyre::{eyre::eyre, Result};
use linked_hash_map::{Entry, LinkedHashMap};
use md5::digest::consts::U16;
use md5::digest::generic_array::GenericArray;
use md5::Digest;
use structopt::StructOpt;

use crate::binnedwrite::Writeable;
use crate::pck::{Pck, PckEntry, MAGIC};

mod binnedwrite;
mod pck;

/// Allows modification of .pck files.
#[derive(StructOpt)]
#[structopt(name = "pck-tweaker")]
struct PckTweaker {
    /// The file to modify. Will be modified in-place.
    #[structopt(parse(from_os_str))]
    pck_file: PathBuf,
    /// The file(s) to add or override.
    /// The filename will be used directly, so the command should be executed in the base directory.
    #[structopt(parse(from_os_str))]
    overlay_files: Vec<PathBuf>,
    /// The alignment for after the entries, and between each file.
    #[structopt(short, long, default_value = "0")]
    alignment: usize,
}

fn main() -> Result<()> {
    color_eyre::install()?;
    let args: PckTweaker = PckTweaker::from_args();
    let pck_file = args.pck_file;
    if !pck_file.exists() {
        return Err(eyre!("missing file: {}", pck_file.display()));
    }

    // Load entire content into memory for editing.
    let pck_file_bytes = std::fs::read(&pck_file)?;

    let pck: Pck = Cursor::new(&pck_file_bytes).read_le()?;

    let mut runtime_entries_by_name = pck
        .entries
        .iter()
        .map(|e| {
            let re = RuntimePckEntry::from_pck_entry(e, |start, end| {
                Vec::from(&pck_file_bytes[start..end])
            });
            (re.name.clone(), re)
        })
        .collect::<LinkedHashMap<_, _>>();

    for file in args.overlay_files {
        let key = file.to_str().expect("Non-UTF8 filename!").to_owned();
        let file_content = std::fs::read(&file)?;
        match runtime_entries_by_name.entry(key.clone()) {
            Entry::Occupied(mut e) => {
                e.get_mut().content = file_content;
            }
            Entry::Vacant(e) => {
                e.insert(RuntimePckEntry {
                    name: key,
                    content: file_content,
                });
            }
        }
    }

    let mut disk_entries: Vec<_> = runtime_entries_by_name
        .into_iter()
        .map(|(_, v)| (v.to_pck_entry(), v.content))
        .collect();

    let mut new_pck_file_bytes = Vec::with_capacity(pck_file_bytes.len());

    new_pck_file_bytes.write_all(MAGIC)?;
    pck.version.write_to(&mut new_pck_file_bytes, ())?;
    pck.godot_version.write_to(&mut new_pck_file_bytes, ())?;
    // Reserved bytes
    new_pck_file_bytes.extend_from_slice(&[0u8; 4 * 16]);
    // Entry count
    (disk_entries.len() as u32).write_to(&mut new_pck_file_bytes, ())?;

    for (e, _) in disk_entries.iter_mut() {
        e.set_offset_pos(new_pck_file_bytes.len() as u64);
        e.write_to(&mut new_pck_file_bytes, ())?;
    }

    while args.alignment != 0 && new_pck_file_bytes.len() % args.alignment != 0 {
        new_pck_file_bytes.push(0);
    }

    for (e, content) in disk_entries {
        let offset_bytes = new_pck_file_bytes.len().to_le_bytes();
        new_pck_file_bytes.extend_from_slice(&content);
        while args.alignment != 0 && new_pck_file_bytes.len() % args.alignment != 0 {
            new_pck_file_bytes.push(0);
        }

        for i in 0..(offset_bytes.len()) {
            new_pck_file_bytes[(e.offset.pos as usize) + i] = offset_bytes[i];
        }
    }

    let tmp_pck_file = pck_file
        .parent()
        .unwrap()
        .join(format!("{}.tmp", pck_file.file_name().unwrap().to_string_lossy()));
    OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&tmp_pck_file)
        .wrap_err_with(|| format!("Failed to open {}", tmp_pck_file.display()))?
        .write_all(&new_pck_file_bytes)?;
    std::fs::rename(tmp_pck_file, pck_file)?;

    Ok(())
}

struct RuntimePckEntry {
    name: String,
    content: Vec<u8>,
}

impl RuntimePckEntry {
    fn from_pck_entry<CF>(pck_entry: &PckEntry, content_fetcher: CF) -> Self
    where
        CF: FnOnce(usize, usize) -> Vec<u8>,
    {
        let content = content_fetcher(
            pck_entry.offset.val as usize,
            (pck_entry.offset.val + pck_entry.size) as usize,
        );
        RuntimePckEntry {
            name: (&pck_entry.name["res://".len()..]).to_owned(),
            content,
        }
    }

    fn to_pck_entry(&self) -> PckEntry {
        let md5_generic = md5::Md5::digest(&self.content);
        let md5 = *RuntimePckEntry::convert(&md5_generic);
        PckEntry {
            name: format!("res://{}", self.name),
            offset: PosValue { val: 0, pos: 0 },
            size: self.content.len() as u64,
            md5,
        }
    }

    // Had to add this from the md5 crate
    #[inline(always)]
    fn convert(d: &GenericArray<u8, U16>) -> &[u8; 16] {
        #[allow(unsafe_code)]
        unsafe {
            &*(d.as_ptr() as *const [u8; 16])
        }
    }
}
