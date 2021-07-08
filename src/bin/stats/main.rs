mod args;

use std::{fs::File, io::BufReader};

use thermal::exif::ThermalExif;
use anyhow::{Result, bail};

use args::Args;

fn main() -> Result<()> {
    let args = Args::from_cmd_line()?;

    let exif: ThermalExif = {
        let exif_file = File::open(&args.exif_path)?;
        let reader = BufReader::new(exif_file);

        let mut values: Vec<_> = serde_json::from_reader(reader)?;
        if values.len() != 1 {
            bail!("expected exif json array with one item");
        }
        values.pop().unwrap()
    };
    eprintln!("{:?}", exif.settings);

    let image = exif.raw.thermal_image()?;
    let (ht, wid) = image.dim();
    eprintln!("image: {}x{}", wid, ht);

    println!("x,y,temp");
    for row in 0..ht {
        for col in 0..wid {
            let raw = image[(row, col)];
            let temp = exif.settings.raw_to_temp(1.0, raw);
            println!("{},{},{}", row, col, temp);
        }
    }

    Ok(())
}
