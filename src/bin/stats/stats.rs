use anyhow::Result;
use rayon::iter::{IntoParallelIterator, ParallelIterator};
use serde_derive::*;
use std::{convert::TryInto, fs::File, io::BufReader, path::Path};
use thermal::{exif::ThermalExiftoolJson, image::ThermalImage, stats::Stats};

#[derive(Serialize, Debug)]
pub struct ImageStats {
    path: String,
    width: usize,
    height: usize,
    pub(crate) stats: Stats,
}

impl ImageStats {
    pub fn from_thermal_image(thermal: &ThermalImage, distance: f64, path: String) -> Result<Self> {
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
            width: wid,
            height: ht,
            stats,
            path,
        })
    }
    pub fn from_image_path(path: &Path, distance: f64) -> Result<Self> {
        let thermal = ThermalImage::from_rjpeg_path(path)?;
        Self::from_thermal_image(&thermal, distance, format!("{}", path.display()))
    }

    pub fn from_exiftool_json_path(path: &Path, distance: f64) -> Result<Vec<Self>> {
        let thermal_exiftool_jsons: Vec<ThermalExiftoolJson> =
            serde_json::from_reader(BufReader::new(File::open(path)?))?;
        thermal_exiftool_jsons
            .into_par_iter()
            .map(move |j| {
                let path = format!("{}", j.source_file.display());
                Self::from_thermal_image(&j.try_into()?, distance, format!("{}", path))
            })
            .collect()
    }
}
