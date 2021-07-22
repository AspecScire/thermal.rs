mod args;
mod stats;

use anyhow::Result;

use args::Args;
use indicatif::{ParallelProgressIterator, ProgressBar, ProgressStyle};
use stats::ImageStats;

fn main() -> Result<()> {
    let args = Args::from_cmd_line()?;

    use rayon::prelude::*;
    use thermal::stats::Stats;

    let bar = ProgressBar::new(args.paths.len() as u64);
    bar.set_style(
        ProgressStyle::default_bar()
            .template("[{elapsed_precise}] {wide_bar:cyan/blue} {pos:>7}/{len:7}"),
    );

    let distance = args.distance;
    let (stats, cumulative) = args
        .paths
        .par_iter()
        .progress_with(bar)
        .map(|p| -> Result<_> {
            if args.is_json {
                ImageStats::from_exiftool_json_path(p, distance)
            } else {
                Ok(vec![ImageStats::from_image_path(p, distance)?])
            }
        })
        .try_fold(
            || (vec![], Stats::default()),
            |mut acc, items| -> Result<_> {
                for item in items? {
                    acc.0.push(item);
                    acc.1 += &acc.0.last().unwrap().stats;
                }
                Ok(acc)
            },
        )
        .try_reduce(
            || (vec![], Stats::default()),
            |mut acc1, acc2| -> Result<_> {
                acc1.0.extend(acc2.0);
                acc1.1 += &acc2.1;
                Ok(acc1)
            },
        )?;

    use serde_derive::*;
    #[derive(Debug, Serialize)]
    struct OutputJson {
        image_stats: Vec<ImageStats>,
        cumulative: Stats,
    }

    serde_json::to_writer(
        std::io::stdout().lock(),
        &OutputJson {
            image_stats: stats,
            cumulative,
        },
    )?;

    Ok(())
}
