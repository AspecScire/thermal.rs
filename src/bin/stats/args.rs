use std::path::PathBuf;
use anyhow::Result;
use thermal::{arg, args_parser};

pub struct Args {
    pub exif_paths: Vec<PathBuf>,
}

impl Args {
    pub fn from_cmd_line() -> Result<Args> {
        let matches = args_parser!("thermal-stats")
            .about("Compute temperature stats from image.")
            .arg(
                arg!("exifs")
                    .required(true)
                    .multiple(true)
                    .help("Exif json path")
            )
            .get_matches();

        let exif_paths = matches.values_of("exifs").unwrap().map(|f| f.into()).collect();

        Ok(Args {
            exif_paths,
        })
    }
}
