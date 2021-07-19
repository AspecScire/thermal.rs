mod args;
mod proc;

use anyhow::Result;
use indicatif::{ParallelProgressIterator, ProgressBar, ProgressStyle};

use crate::{
    args::Args,
    proc::{copy_exif_and_xmp, transform_image_tiff, TransformArgs},
};

fn main() -> Result<()> {
    let args = Args::from_cmd_line()?;

    let bar = ProgressBar::new(args.paths.len() as u64);
    bar.set_style(
        ProgressStyle::default_bar()
            .template("[{elapsed_precise}] {wide_bar:cyan/blue} {pos:>7}/{len:7}"),
    );

    let t_args = TransformArgs::from_args(&args);

    use rayon::prelude::*;
    let count = args
        .paths
        .par_iter()
        .progress_with(bar)
        .map(|p| -> Result<()> {
            let out_path = transform_image_tiff(p, &t_args)?;
            if args.copy_exif {
                copy_exif_and_xmp(p, &out_path)?;
            }
            Ok(())
        })
        .try_fold(
            || 0usize,
            |acc, res| -> Result<_> {
                res?;
                Ok(acc + 1)
            },
        )
        .try_reduce(|| 0, |a, b| Ok(a + b))?;

    eprintln!("Processed {} images", count);
    eprintln!(
        "Transform equation: V = {} + {} C",
        t_args.coeffs[0], t_args.coeffs[1]
    );
    eprintln!(
        "Inverse equation: C = {} + {} V",
        -t_args.coeffs[0] / t_args.coeffs[1],
        1. / t_args.coeffs[1]
    );
    Ok(())
}
