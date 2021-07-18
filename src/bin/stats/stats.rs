use std::{fs::read, path::Path};

use img_parts::jpeg::Jpeg;
use thermal::{flir::FlirSegment, stats::Stats, temperature::ThermalSettings};
use anyhow::{Result, anyhow};
use serde_derive::*;


#[derive(Serialize, Debug)]
pub struct ImageStats {
    path: String,
    width: usize,
    height: usize,
    pub(crate) stats: Stats,
}

impl ImageStats {
    pub fn from_image_path(path: &Path, distance: f64) -> Result<Self> {
        let flir_segment = {
            let image = Jpeg::from_bytes(read(path)?.into())?;
            FlirSegment::try_from_jpeg(&image)?
        };
        let image = flir_segment.try_parse_raw_data()?
            .ok_or_else(|| anyhow!("no raw data found"))?;
        let (ht, wid) = image.dim();

        let thermal_settings: ThermalSettings = flir_segment.try_parse_camera_params()?
            .ok_or_else(|| anyhow!("no camera params found"))?
            .into();
        let temp_t = thermal_settings.temperature_transform(distance);

        let mut stats = Stats::default();
        for row in 0..ht {
            for col in 0..wid {
                let raw = image[(row, col)] as f64;
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
