use super::Args;
use anyhow::{ensure, Result};
use byteordered::ByteOrdered;
use image::tiff::TiffEncoder;
use img_parts::jpeg::Jpeg;
use itertools::iproduct;
use std::{
    fs::{read, File},
    io::{BufWriter, Cursor},
    path::{Path, PathBuf},
    process::Command,
};
use thermal::image::ThermalImage;

pub struct TransformArgs {
    pub distance: f64,
    pub coeffs: [f64; 2],
    pub output: PathBuf,
}

impl TransformArgs {
    pub fn from_args(args: &Args) -> Self {
        let factor = u16::MAX as f64 / (args.max - args.min);
        let coeffs = [-args.min * factor, factor];

        TransformArgs {
            distance: args.distance,
            coeffs,
            output: args.output.clone(),
        }
    }

    pub fn transform(&self, val: f64) -> u16 {
        let tval = self.coeffs[0] + self.coeffs[1] * val;
        tval.max(0.).min(u16::MAX as f64) as u16
    }

    pub fn output_stem_for(&self, path: &Path) -> PathBuf {
        self.output.join(path.file_stem().unwrap())
    }
}

fn image_to_u16_iterator<'a>(
    thermal: &'a ThermalImage,
    args: &'a TransformArgs,
) -> Result<impl Iterator<Item = (usize, usize, u16)> + 'a> {
    let temp_t = thermal.settings.temperature_transform(args.distance);
    let (ht, wid) = thermal.image.dim();
    Ok(iproduct!(0..ht, 0..wid).map(move |(row, col)| {
        let tval = args.transform(temp_t(thermal.image[(row, col)] as f64));
        (row, col, tval)
    }))
}

pub fn transform_image_tiff(path: &Path, args: &TransformArgs) -> Result<PathBuf> {
    let image = Jpeg::from_bytes(read(path)?.into())?;
    let thermal = ThermalImage::from_rjpeg(&image)?;

    let (ht, wid) = thermal.image.dim();
    let mut image_buffer = {
        let vec = Vec::with_capacity(2 * ht * wid);
        let cursor = Cursor::new(vec);
        ByteOrdered::native(cursor)
    };
    for (_, _, val) in image_to_u16_iterator(&thermal, args)? {
        image_buffer.write_u16(val)?;
    }

    let output_path = args.output_stem_for(path).with_extension("tif");
    let image_writer = BufWriter::new(File::create(&output_path)?);
    TiffEncoder::new(image_writer).encode(
        &image_buffer.into_inner().into_inner(),
        wid as u32,
        ht as u32,
        image::ColorType::L16,
    )?;

    Ok(output_path)
}

#[allow(dead_code)]
pub fn transform_image_png(path: &Path, args: &TransformArgs) -> Result<PathBuf> {
    let image = Jpeg::from_bytes(read(path)?.into())?;
    let thermal = ThermalImage::from_rjpeg(&image)?;

    let outpath = args.output_stem_for(path).with_extension("png");
    let image_writer = BufWriter::new(File::create(&outpath)?);
    let mut png_writer = {
        let (ht, wid) = thermal.image.dim();
        let mut encoder = png::Encoder::new(image_writer, wid as u32, ht as u32);
        encoder.set_color(png::ColorType::Grayscale);
        encoder.set_depth(png::BitDepth::Sixteen);
        encoder.write_header()?
    };
    let mut png_streamer = ByteOrdered::be(png_writer.stream_writer());

    for (_, _, val) in image_to_u16_iterator(&thermal, args)? {
        png_streamer.write_u16(val)?;
    }
    png_streamer.into_inner().finish()?;

    Ok(outpath)
}

pub fn copy_exif_and_xmp(path: &Path, output_path: &Path) -> Result<()> {
    ensure!(
        Command::new("sh")
            .arg("-c")
            .arg(&format!(
                "exiv2 -ea- {:?} | exiv2 -ia- {:?}",
                path, output_path,
            ))
            .status()?
            .success(),
        "failed to copy exif from input image"
    );

    ensure!(
        Command::new("sh")
            .arg("-c")
            .arg(&format!(
                "exiv2 -eX- {:?} | exiv2 -iX- {:?}",
                path, output_path,
            ))
            .status()?
            .success(),
        "failed to copy xmp from input image"
    );

    Ok(())
}
