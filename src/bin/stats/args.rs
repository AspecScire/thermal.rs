use anyhow::Result;
use clap::value_t_or_exit;
use std::path::PathBuf;
use thermal::{arg, args_parser, opt};

pub struct Args {
    pub paths: Vec<PathBuf>,
    pub distance: f64,
}

impl Args {
    pub fn from_cmd_line() -> Result<Args> {
        let matches = args_parser!("thermal-stats")
            .about("Compute temperature stats from image.")
            .arg(
                arg!("images")
                    .required(true)
                    .multiple(true)
                    .help("Image paths"),
            )
            .arg(
                opt!("distance")
                    .short("d")
                    .help("Distance to use for calculation.  Default is 1.0"),
            )
            .get_matches();

        let paths = matches
            .values_of("images")
            .unwrap()
            .map(|f| f.into())
            .collect();
        let distance = matches
            .is_present("distance")
            .then(|| value_t_or_exit!(matches.value_of("distance"), f64))
            .unwrap_or(1.0);

        Ok(Args { paths, distance })
    }
}
