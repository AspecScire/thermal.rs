use anyhow::Result;
use clap::value_t_or_exit;
use std::path::PathBuf;
use thermal::{arg, args_parser, opt};

pub struct Args {
    pub paths: Vec<PathBuf>,
    pub output: PathBuf,
    pub min: f64,
    pub max: f64,
    pub distance: f64,
    pub copy_exif: bool,
}

impl Args {
    pub fn from_cmd_line() -> Result<Args> {
        let matches = args_parser!("thermal-stats")
            .setting(clap::AppSettings::AllowLeadingHyphen)
            .about("Compute temperature stats from image.")
            .arg(
                opt!("output")
                    .required(true)
                    .help("Min value for transform"),
            )
            .arg(opt!("min").required(true).help("Min value for transform"))
            .arg(opt!("max").required(true).help("Max value for transform"))
            .arg(
                opt!("copy exif")
                    .takes_value(false)
                    .short("x")
                    .help("Copy exif from source file to the target (requires exiv2)"),
            )
            .arg(
                opt!("distance")
                    .short("d")
                    .help("Distance to use for calculation.  Default is 1.0"),
            )
            .arg(
                arg!("images")
                    .required(true)
                    .multiple(true)
                    .help("Image paths"),
            )
            .get_matches();

        let paths = matches
            .values_of("images")
            .unwrap()
            .map(|f| f.into())
            .collect();
        let output = value_t_or_exit!(matches, "output", PathBuf);
        let min = value_t_or_exit!(matches, "min", f64);
        let max = value_t_or_exit!(matches, "max", f64);
        let distance = matches
            .is_present("distance")
            .then(|| value_t_or_exit!(matches.value_of("distance"), f64))
            .unwrap_or(1.0);

        let copy_exif = matches.is_present("copy exif");

        Ok(Args {
            paths,
            output,
            distance,
            min,
            max,
            copy_exif,
        })
    }
}
