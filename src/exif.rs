//! Parse values and parameters from output of `exiftool -j
//! -b`.

use std::path::PathBuf;

use serde_derive::*;

use crate::{image::ThermalRawBytes, temperature::ThermalSettings};

#[derive(Serialize, Deserialize, Debug)]
pub struct ThermalExiftoolJson {
    #[serde(rename = "SourceFile")]
    pub source_file: PathBuf,

    #[serde(flatten)]
    pub settings: ThermalSettings,

    #[serde(flatten)]
    pub raw: ThermalRawBytes,
}
