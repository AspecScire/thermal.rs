use anyhow::Result;
use serde_derive::*;
use std::path::Path;
use thermal::{image::ThermalImage, stats::Stats};

#[derive(Serialize, Debug)]
pub struct ImageStats {
    path: String,
    width: usize,
    height: usize,
    pub(crate) stats: Stats,
}

impl ImageStats {
    pub fn from_image_path(path: &Path, distance: f64) -> Result<Self> {
        let thermal = ThermalImage::from_rjpeg_path(path)?;
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
        Ok(ImageStats {
            path: format!("{}", path.display()),
            width: wid,
            height: ht,
            stats,
        })
    }
}
