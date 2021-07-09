use std::{fs::File, io::BufReader, path::Path};

use thermal::{exif::ThermalExif, stats::Stats};
use anyhow::{Result, bail};
use serde_derive::*;


#[derive(Serialize, Debug)]
pub struct ImageStats {
    path: String,
    width: usize,
    height: usize,
    pub(crate) stats: Stats,
}

impl ImageStats {
    pub fn from_exif_path(path: &Path, distance: f64) -> Result<Self> {
        let exif: ThermalExif = {
            let exif_file = File::open(path)?;
            let reader = BufReader::new(exif_file);

            let mut values: Vec<_> = serde_json::from_reader(reader)?;
            if values.len() != 1 {
                bail!("expected exif json array with one item");
            }
            values.pop().unwrap()
        };

        let image = exif.raw.thermal_image()?;
        let (ht, wid) = image.dim();

        let mut stats = Stats::default();
        let temp_t = exif.settings.temperature_transform(distance);

        for row in 0..ht {
            for col in 0..wid {
                let raw = image[(row, col)];
                let temp = temp_t(raw);
                stats += temp;
            }
        }
        Ok(ImageStats {
            path: format!("{}", path.display()),
            width: wid,
            height: ht,
            stats,
        })
    }
}
