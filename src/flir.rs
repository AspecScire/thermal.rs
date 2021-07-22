//! Parse parameters from FLIR R-JPEGs.
//!
//! This is an incomplete port of relevant parts of the
//! excellent [ExifTool] by Phil Harvey and other authors.
//! Currently only supports R-JPEGs with FFF headers, with
//! 16-bit raw image, and only reads enough parameters to be
//! able to compute [`temperature`][crate::temperature] values
//! from the raw sensor values.
//!
//! For a more complete extraction of FLIR and other
//! metadata, please use [ExifTool] directly. The JSON
//! output from `exiftool -j -b` can also be used directly
//! to compute the temperature values via the
//! [`exif`][crate::exif] module.
//!
//! [ExifTool]: //exiftool.org
use std::io::Read;

use anyhow::{anyhow, bail, ensure, Result};
use bincode::{deserialize, DefaultOptions, Options};
use img_parts::jpeg::{markers, Jpeg};
use ndarray::Array2;
use serde::Deserialize;
use serde_derive::*;

/// Data collected from the FLIR segment(s) of an R-JPEG.
#[derive(Debug)]
pub struct FlirSegment {
    data: Vec<u8>,
    dir: Vec<FlirRecordDirEntry>,
}

impl FlirSegment {
    /// Tries to collect all the FLIR segments from an
    /// [`Jpeg`] image and parse the FLIR header from it.
    /// Returns a `FlirSegment` if both steps are
    /// successful.
    pub fn try_from_jpeg(image: &Jpeg) -> Result<Self> {
        let data = collect_flir_segment_data_from_jpeg(image)?;
        let dir = parse_flir_segment(&data)?;
        Ok(FlirSegment { data, dir })
    }

    /// Try to find and parse raw sensor values as a 2-D
    /// array. Returns the raw values as a 2-D array of
    /// `f64`s if found, and `None` if not found (but the
    /// parsing was otherwise successful).
    pub fn try_parse_raw_data(&self) -> Result<Option<Array2<f64>>> {
        self.dir
            .iter()
            .find_map(|e| e.try_parse_raw_data(&self.data).transpose())
            .transpose()
    }

    /// Try to find and parse the FLIR camera parameters
    /// from the data. Returns `None` if not found.
    pub fn try_parse_camera_params(&self) -> Result<Option<FlirCameraParams>> {
        self.dir
            .iter()
            .find_map(|e| e.try_parse_camera_params(&self.data).transpose())
            .transpose()
    }
}

/// Collect FLIR data from Jpeg APP1 segments.
///
/// # Implementation
///
/// FLIR data is stored as a collection of APP1 segments
/// with the following format:
///
/// - 0x0: signature: "FLIR\0"
/// - 0x6: segment number: zero-based idx
/// - 0x7: last segment number (= total segments - 1)
/// - 0x8..: data
fn collect_flir_segment_data_from_jpeg(image: &Jpeg) -> Result<Vec<u8>> {
    let mut flir_segments: Vec<Vec<u8>> = vec![];
    let mut num_copied = 0;
    let mut total_len = 0;

    for segment in image.segments_by_marker(markers::APP1) {
        let contents = segment.contents();
        if contents.len() < 8 || &contents[0..5] != b"FLIR\0" {
            continue;
        }

        let current_segment = contents[6] as usize;
        let total_segments = contents[7] as usize + 1;

        match flir_segments.len() {
            0 => flir_segments.resize(total_segments, vec![]),
            l if l != total_segments => bail!(
                "inconsistent count of total FLIR segments: {} != {}",
                l,
                total_segments
            ),
            l if l <= current_segment as usize => bail!(
                "FLIR segment idx out of bounds: {} >= {}",
                current_segment,
                l
            ),
            _ => (),
        }

        let curr_seg = &mut flir_segments[current_segment as usize];
        ensure!(
            curr_seg.len() == 0,
            "duplicate FLIR segment: idx = {}",
            current_segment
        );

        curr_seg.extend_from_slice(&contents[8..]);
        num_copied += 1;
        total_len += curr_seg.len();
    }

    ensure!(
        num_copied == flir_segments.len(),
        "expected {} FLIR segments, found only {}",
        flir_segments.len(),
        num_copied
    );

    let mut flir_data = Vec::with_capacity(total_len);
    for seg in flir_segments {
        flir_data.extend_from_slice(&seg);
    }
    Ok(flir_data)
}

fn is_segment_little_endian(segment: &[u8]) -> Result<bool> {
    #[derive(Debug, Deserialize)]
    struct FlirHeaderPre {
        format: [u8; 4],
        creator: [u8; 16],
        version: u32,
    }

    let hdr: FlirHeaderPre = DefaultOptions::new()
        .with_little_endian()
        .with_fixint_encoding()
        .allow_trailing_bytes()
        .deserialize(&segment)?;

    if &hdr.format != b"FFF\0" {
        bail!("unexpected signature in FLIR segment");
    }

    Ok(hdr.version >= 100 && hdr.version < 200)
}
fn parse_flir_segment(segment: &[u8]) -> Result<Vec<FlirRecordDirEntry>> {
    let is_le = is_segment_little_endian(segment)?;

    // # FLIR file header (ref 3)
    // # 0x00 - string[4] file format ID = "FFF\0"
    // # 0x04 - string[16] file creator: seen "\0","MTX IR\0","CAMCTRL\0"
    // # 0x14 - int32u file format version = 100
    // # 0x18 - int32u offset to record directory
    // # 0x1c - int32u number of entries in record directory
    // # 0x20 - int32u next free index ID = 2
    // # 0x24 - int16u swap pattern = 0 (?)
    // # 0x28 - int16u[7] spares
    // # 0x34 - int32u[2] reserved
    // # 0x3c - int32u checksum
    #[derive(Debug, Deserialize)]
    struct FlirHeader {
        offset: u32,
        num_entries: u32,

        next_free: u32,
        swap_pattern: u16,
        spares: [u16; 7],
        reserved: [u32; 2],
        checksum: u32,
    }

    let hdr: FlirHeader = deserialize_with_endian(is_le, &segment[0x18..])?;
    let mut dir_segment = &segment[hdr.offset as usize..];

    (0..hdr.num_entries)
        .map(|_| deserialize_with_endian(is_le, &mut dir_segment))
        .collect()
}

// # FLIR record entry (ref 3):
// # 0x00 - int16u record type
// # 0x02 - int16u record subtype: RawData 1=BE, 2=LE, 3=PNG; 1 for other record types
// # 0x04 - int32u record version: seen 0x64,0x66,0x67,0x68,0x6f,0x104
// # 0x08 - int32u index id = 1
// # 0x0c - int32u record offset from start of FLIR data
// # 0x10 - int32u record length
// # 0x14 - int32u parent = 0 (?)
// # 0x18 - int32u object number = 0 (?)
// # 0x1c - int32u checksum: 0 for no checksum
#[derive(Debug, Deserialize)]
pub struct FlirRecordDirEntry {
    ty: u16,
    sub_type: u16,
    version: u32,

    id: u32,
    offset: u32,
    length: u32,

    parent: u32,
    obj_num: u32,
    checksum: u32,
}
impl FlirRecordDirEntry {
    pub fn data<'a>(&self, segment: &'a [u8]) -> Option<&'a [u8]> {
        segment.get(self.offset as usize..(self.offset + self.length) as usize)
    }
    pub fn try_parse_raw_data(&self, segment: &[u8]) -> Result<Option<Array2<f64>>> {
        if self.ty != 0x01 {
            return Ok(None);
        }
        ensure!(self.sub_type != 3, "PNG type raw data not yet supported");

        let data = self
            .data(segment)
            .ok_or_else(|| anyhow!("unexpected end of FLIR segment while reading record"))?;

        ensure!(
            data.len() > 6,
            "raw data record size mismatch: expected at least 6 bytes, found {}",
            data.len(),
        );
        let is_le = deserialize_with_endian::<u16, _>(true, &data[..])? == 2;

        #[derive(Debug, Deserialize)]
        struct RawDataDims {
            width: u16,
            height: u16,
        }
        let dims: RawDataDims = deserialize_with_endian(is_le, &data[2..])?;
        let width: usize = dims.width as usize;
        let height: usize = dims.height as usize;
        let expected = 2 * (16 + width * height);

        ensure!(
            data.len() == expected,
            "raw data record size mismatch: expected {} bytes, found {}",
            expected,
            data.len()
        );

        let mut raw_data_slice = &data[0x20..];
        let mut raw_data = Vec::with_capacity(width * height);
        for _ in 0..height {
            for _ in 0..width {
                raw_data
                    .push(deserialize_with_endian::<u16, _>(is_le, &mut raw_data_slice)? as f64);
            }
        }

        Ok(Some(Array2::from_shape_vec((height, width), raw_data)?))
    }

    pub fn try_parse_camera_params(&self, segment: &[u8]) -> Result<Option<FlirCameraParams>> {
        if self.ty != 0x20 {
            return Ok(None);
        }

        let data = self
            .data(segment)
            .ok_or_else(|| anyhow!("unexpected end of FLIR segment while reading record"))?;

        ensure!(
            data.len() >= 0x384,
            "raw data record size mismatch: expected at least {} bytes, found {}",
            0x384,
            data.len()
        );

        let is_le = deserialize_with_endian::<u16, _>(true, &data[..])? == 2;

        let temperature_params: FlirTemperatureParams =
            deserialize_with_endian(is_le, &data[0x20..])?;
        let camera_info: FlirCameraInfo = deserialize_with_endian(is_le, &data[0xd4..])?;
        let lens_info: FlirLensInfo = deserialize_with_endian(is_le, &data[0x170..])?;
        let filter_info: FlirFilterInfo = deserialize_with_endian(is_le, &data[0x1ec..])?;
        let extra_params: FlirExtraParams = deserialize_with_endian(is_le, &data[0x308..])?;

        Ok(Some(FlirCameraParams {
            temperature_params,
            camera_info,
            lens_info,
            filter_info,
            extra_params,
        }))
    }
}

#[derive(Debug, Deserialize)]
pub struct FlirCameraParams {
    pub(crate) temperature_params: FlirTemperatureParams,
    pub(crate) camera_info: FlirCameraInfo,
    pub(crate) lens_info: FlirLensInfo,
    pub(crate) filter_info: FlirFilterInfo,
    pub(crate) extra_params: FlirExtraParams,
}

#[derive(Debug, Deserialize)]
pub struct FlirTemperatureParams {
    pub(crate) emissivity: f32,
    pub(crate) object_distance: f32,

    pub(crate) reflected_apparent_temperature: f32,
    pub(crate) atmospheric_temperature: f32,
    pub(crate) ir_window_temperature: f32,
    pub(crate) ir_window_transmission: f32,

    _dummy_ignore: u32,

    pub(crate) relative_humidity: f32,
    _dummy_ignore_1: [u32; 6],

    pub(crate) planck_r1: f32,
    pub(crate) planck_b: f32,
    pub(crate) planck_f: f32,
    _dummy_ignore_2: [u32; 3],

    pub(crate) atmospheric_transmission_alpha_1: f32,
    pub(crate) atmospheric_transmission_alpha_2: f32,
    pub(crate) atmospheric_transmission_beta_1: f32,
    pub(crate) atmospheric_transmission_beta_2: f32,
    pub(crate) atmospheric_transmission_x: f32,
    _dummy_ignore_3: [u32; 3],

    pub(crate) camera_temperature_range: [f32; 8],
}

#[derive(Debug, Deserialize)]
pub struct FlirCameraInfo {
    pub(crate) camera_mode: [u8; 32],
    pub(crate) camera_part_number: [u8; 16],
    pub(crate) camera_serial_number: [u8; 16],
    pub(crate) camera_software: [u8; 16],
}

#[derive(Debug, Deserialize)]
pub struct FlirLensInfo {
    pub(crate) lens_mode: [u8; 32],
    pub(crate) lens_part_number: [u8; 16],
    pub(crate) lens_serial_number: [u8; 16],
}

#[derive(Debug, Deserialize)]
pub struct FlirFilterInfo {
    pub(crate) filter_mode: [u8; 32],
    pub(crate) filter_part_number: [u8; 16],
    pub(crate) filter_serial_number: [u8; 16],
}

#[derive(Debug, Deserialize)]
pub struct FlirExtraParams {
    pub(crate) planck_o: i32,
    pub(crate) planck_r2: f32,
    pub(crate) raw_value_ranges: [u16; 4],
}

fn deserialize_with_endian<'a, T, R>(use_little_endian: bool, read: R) -> Result<T>
where
    T: for<'de> Deserialize<'de>,
    R: Read,
{
    let opts = DefaultOptions::new()
        .with_fixint_encoding()
        .allow_trailing_bytes();
    Ok(if use_little_endian {
        opts.with_little_endian().deserialize_from(read)?
    } else {
        opts.with_big_endian().deserialize_from(read)?
    })
}
