//! Helper macros for parsing packed structs.
//!
//! The format is pretty much like `bincode` for structs,
//! but we do not want to rely on bincode just for this.
use byteordered::{ByteOrdered, Endian, byteorder::ReadBytesExt};

macro_rules! decode_from_buffer {
    ($rdr: expr, $( $ty:ty => $err:expr ),* $(,)?) => {{
        let mut rdr = $rdr;
        (
            $(decode_one_with_context!(&mut rdr, $ty, $err)),*
        )
    }};
}

macro_rules! decode_one_with_context {
	  ($rdr:expr, $ty:ty, $err:expr) => {
        anyhow::Context::with_context(
            <$ty as crate::parse::Parseable>::parse($rdr),
            || format!("parsing {}", $err),
        )?
	  };
}

#[test]
fn test_decode_from_buffer() -> anyhow::Result<()> {
    let slice: Vec<u8> = vec![0; 2 + 4 + 8 + 4 + 3];
    let mut rdr = ByteOrdered::native(&slice[0x2..]);
    let (_, _, _) = decode_from_buffer!(
        &mut rdr,
        u32 => "foo",
        u64 => "bar",
        [u8; 4] => "dummy",
    );
    let _ = decode_from_buffer!(
        rdr,
        u32 => "blah",
    );
    Ok(())
}

use std::io::Result as IOResult;
pub(crate) trait Parseable: Sized {
    fn parse<T: ReadBytesExt, E: Endian>(r: &mut ByteOrdered<T, E>) -> IOResult<Self>;
}

macro_rules! impl_parseable {
	  ($ty:ty, $method:ident) => {
        impl Parseable for $ty {
            fn parse<T: ReadBytesExt, E: Endian>(r: &mut ByteOrdered<T, E>) -> IOResult<Self> {
                r.$method()
            }
        }
	  };
}

impl_parseable!(u8, read_u8);
impl_parseable!(i8, read_i8);
impl_parseable!(u16, read_u16);
impl_parseable!(i16, read_i16);
impl_parseable!(u32, read_u32);
impl_parseable!(i32, read_i32);
impl_parseable!(u64, read_u64);
impl_parseable!(i64, read_i64);

impl<Ty, const N: usize> Parseable for [Ty; N] where Ty: Parseable, [Ty; N]: Default {
    fn parse<T: ReadBytesExt, E: Endian>(r: &mut ByteOrdered<T, E>) -> IOResult<Self> {
        let mut out: [Ty; N] = Default::default();
        for i in 0..N {
            out[i] = Ty::parse(r)?;
        }
        Ok(out)
    }
}
