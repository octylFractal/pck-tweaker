//! A version of what BinWrite (no derive) might look like.
use std::io::Write;

pub trait Writeable {
    type Args;

    fn write_to<W: Write>(&self, write: &mut W, args: Self::Args) -> std::io::Result<()>;
}

impl Writeable for u32 {
    type Args = ();

    fn write_to<W: Write>(&self, write: &mut W, _: Self::Args) -> std::io::Result<()> {
        write.write_all(&self.to_le_bytes())
    }
}

impl Writeable for u64 {
    type Args = ();

    fn write_to<W: Write>(&self, write: &mut W, _: Self::Args) -> std::io::Result<()> {
        write.write_all(&self.to_le_bytes())
    }
}

impl Writeable for str {
    type Args = ();

    fn write_to<W: Write>(&self, write: &mut W, _: Self::Args) -> std::io::Result<()> {
        let bytes = self.as_bytes();
        (bytes.len() as u32).write_to(write, ())?;

        write.write_all(bytes)?;

        Ok(())
    }
}
