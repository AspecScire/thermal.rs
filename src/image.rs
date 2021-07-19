use std::{fs::read, io::Cursor, path::Path};

use anyhow::{anyhow, bail, Result};
use image::{ColorType, ImageDecoder};
use img_parts::jpeg::Jpeg;
use ndarray::Array2;
use serde_derive::*;

use crate::{flir::FlirSegment, temperature::ThermalSettings};

pub struct ThermalImage {
    pub settings: ThermalSettings,
    pub image: Array2<u16>,
}
impl ThermalImage {
    pub fn from_rjpeg(image: &Jpeg) -> Result<Self> {
        let flir_segment = FlirSegment::try_from_jpeg(&image)?;
        let image = flir_segment
            .try_parse_raw_data()?
            .ok_or_else(|| anyhow!("no raw data found"))?;
        let settings: ThermalSettings = flir_segment
            .try_parse_camera_params()?
            .ok_or_else(|| anyhow!("no camera params found"))?
            .into();
        Ok(ThermalImage { image, settings })
    }

    pub fn from_rjpeg_path(path: &Path) -> Result<Self> {
        let image = Jpeg::from_bytes(read(path)?.into())?;
        Self::from_rjpeg(&image)
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct ThermalRawBytes {
    #[serde(rename = "RawThermalImageType")]
    ty: String,

    #[serde(
        rename = "RawThermalImage",
        deserialize_with = "serde_helpers::base64_bytes"
    )]
    base64_bytes: Vec<u8>,
}

impl ThermalRawBytes {
    pub fn thermal_image(&self) -> Result<Array2<f64>> {
        if self.ty != "TIFF" {
            bail!("unsupported image type: {}", self.ty);
        }

        use image::tiff::TiffDecoder;
        let decoder = TiffDecoder::new(Cursor::new(&self.base64_bytes))?;
        let (width, height) = decoder.dimensions();
        let width = width as usize;
        let height = height as usize;
        let depth = match decoder.color_type() {
            ColorType::L8 => 8,
            ColorType::L16 => 16,
            _ => bail!("supported color type: {:?}", decoder.color_type()),
        };

        use zerocopy::{AsBytes, FromBytes};
        fn image_as_float<'a, T, R>(decoder: R) -> Result<Vec<f64>>
        where
            f64: From<T>,
            T: AsBytes + FromBytes,
            R: ImageDecoder<'a>,
        {
            let (width, height) = decoder.dimensions();
            let num_pixels = (width * height) as usize;
            let mut image: Vec<T> = Vec::with_capacity(num_pixels);
            unsafe {
                image.set_len(num_pixels);
            }
            decoder.read_image(image.as_bytes_mut())?;
            Ok(image.into_iter().map(|f| f.into()).collect())
        }

        let output = if depth == 8 {
            image_as_float::<u8, _>(decoder)?
        } else if depth == 16 {
            image_as_float::<u16, _>(decoder)?
        } else {
            unreachable!("unexpected depth: {}", depth);
        };

        Ok(Array2::from_shape_vec((height, width), output)?)
    }
}

mod serde_helpers {
    use lazy_static::lazy_static;
    use regex::Regex;
    use serde::*;

    pub fn base64_bytes<'de, D>(de: D) -> Result<Vec<u8>, D::Error>
    where
        D: Deserializer<'de>,
    {
        lazy_static! {
            static ref RE: Regex = Regex::new(r"^base64:").unwrap();
        }

        use serde::de::Error;
        let str_rep = <String as Deserialize>::deserialize(de)?;

        RE.find(&str_rep).ok_or(Error::custom(
            "unexpected format: must begin with `base64:`",
        ))?;

        use base64::decode;
        let slice = &str_rep[7..];
        let bytes = decode(slice).map_err(Error::custom)?;

        Ok(bytes)
    }
}
