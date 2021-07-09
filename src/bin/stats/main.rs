mod args;
mod stats;

use anyhow::{Result, bail};

use args::Args;
use stats::ImageStats;

fn main() -> Result<()> {
    let args = Args::from_cmd_line()?;

    use rayon::prelude::*;
    use thermal::stats::PixelStats;
    let (stats, cumulative) = args.exif_paths
        .into_par_iter()
        .map(|p| ImageStats::from_exif_path(&p))
        .try_fold(
            || (vec![], PixelStats::default()),
            |mut acc, item| -> Result<_> {
                acc.0.push(item?);
                acc.1 += &acc.0.last().unwrap().stats;
                Ok(acc)
            }
        )
        .try_reduce(
            || (vec![], PixelStats::default()),
            |mut acc1, acc2| -> Result<_> {
                acc1.0.extend(acc2.0);
                acc1.1 += &acc2.1;
                Ok(acc1)
            }
        )?;


    use serde_derive::*;
    #[derive(Debug, Serialize)]
    struct OutputJson {
        image_stats: Vec<ImageStats>,
        cumulative: PixelStats,
    }

    serde_json::to_writer(
        std::io::stdout().lock(),
        &OutputJson { image_stats: stats, cumulative, },
    )?;

    Ok(())
}
