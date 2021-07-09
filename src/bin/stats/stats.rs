use std::{fs::File, io::BufReader, path::Path};

use thermal::{exif::ThermalExif, stats::PixelStats};
use anyhow::{Result, bail};
use serde_derive::*;


#[derive(Serialize, Debug)]
pub struct ImageStats {
    path: String,
    width: usize,
    height: usize,
    pub(crate) stats: PixelStats,
}

impl ImageStats {
    pub fn from_exif_path(path: &Path) -> Result<Self> {
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

        let mut stats = PixelStats::default();
        for row in 0..ht {
            for col in 0..wid {
                let raw = image[(row, col)];
                let temp = exif.settings.raw_to_temp(1.0, raw);
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
