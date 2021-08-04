mod args;

use anyhow::Result;
use args::Args;
use serde_derive::*;

use thermal::cli::process_paths_par;
use thermal::{image::ThermalImage, stats::Stats};

fn main() -> Result<()> {
    let args = Args::from_cmd_line()?;

    use rayon::prelude::*;

    let Args {
        paths,
        distance,
        is_json,
    } = args;

    let (stats, cumulative) = process_paths_par(paths, is_json)
        .into_par_iter()
        .map(|try_img| -> Result<_> {
            let img = try_img?;
            Ok(ImageStats::from_thermal_image(&img.image, distance, img.filename))
        })
        .try_fold(
            || (vec![], Stats::default()),
            |mut acc, try_img| -> Result<_> {
                let item = try_img?;
                acc.0.push(item);
                acc.1 += &acc.0.last().unwrap().stats;
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

#[derive(Serialize, Debug)]
pub struct ImageStats {
    path: String,
    width: usize,
    height: usize,
    pub(crate) stats: Stats,
}

impl ImageStats {
    pub fn from_thermal_image(thermal: &ThermalImage, distance: f64, path: String) -> Self {
        let temp_t = thermal.settings.temperature_transform(distance);
        let (ht, wid) = thermal.image.dim();

        let mut stats = Stats::default();
        for row in 0..ht {
            for col in 0..wid {
                let raw = thermal.image[(row, col)] as f64;
                let temp = temp_t(raw);
                stats += temp;
            }
        }
        ImageStats {
            width: wid,
            height: ht,
            path,
            stats,
        }
    }
}
