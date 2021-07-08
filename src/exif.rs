use serde_derive::*;

use crate::{image::ThermalRawBytes, temperature::ThermalSettings};

#[derive(Serialize, Deserialize, Debug)]
pub struct ThermalExif {
    #[serde(flatten)]
    pub settings: ThermalSettings,

    #[serde(flatten)]
    pub raw: ThermalRawBytes,
}
