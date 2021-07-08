use std::path::PathBuf;
use anyhow::Result;
use thermal::{arg, args_parser};

pub struct Args {
    pub image_path: PathBuf,
    pub exif_path: PathBuf,
}

impl Args {
    pub fn from_cmd_line() -> Result<Args> {
        use clap::*;
        let matches = args_parser!("thermal-stats")
            .about("Compute temperature stats from image.")
            .arg(
                arg!("image")
                    .required(true)
                    .help("Image path (R-JPEG image)"),
            )
            .arg(
                arg!("exif")
                    .required(true)
                    .help("Exif json path")
            )
            .get_matches();

        let image_path = value_t!(matches, "image", PathBuf)?;
        let exif_path = value_t!(matches, "exif", PathBuf)?;

        Ok(Args {
            image_path, exif_path,
        })
    }
}
