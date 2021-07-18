use std::io::Read;

use anyhow::{anyhow, bail, ensure, Result};
use bincode::{deserialize, DefaultOptions, Options};
use img_parts::jpeg::{markers, Jpeg};
use ndarray::Array2;
use serde::Deserialize;
use serde_derive::*;

#[derive(Debug)]
pub struct FlirSegment {
    data: Vec<u8>,
    dir: Vec<FlirRecordDirEntry>,
}
impl FlirSegment {
    pub fn size(&self) -> usize {
        self.data.len()
    }
    pub fn num_records(&self) -> usize {
        self.dir.len()
    }
}

impl FlirSegment {
    pub fn try_from_jpeg(image: &Jpeg) -> Result<Self> {
        let data = extract_flir_segment_from_jpeg(image)?;
        let dir = parse_flir_segment(&data)?;
        Ok(FlirSegment { data, dir })
    }
    pub fn try_parse_raw_data(&self) -> Result<Option<Array2<u16>>> {
        self.dir
            .iter()
            .find_map(|e| e.try_parse_raw_data(&self.data).transpose())
            .transpose()
    }
    pub fn try_parse_camera_params(&self) -> Result<Option<FlirCameraParams>> {
        self.dir
            .iter()
            .find_map(|e| e.try_parse_camera_params(&self.data).transpose())
            .transpose()
    }
}

fn extract_flir_segment_from_jpeg(image: &Jpeg) -> Result<Vec<u8>> {
    #[derive(Deserialize)]
    struct FlirSegment {
        signature: [u8; 5],
        _dummy: u8,
        current_segment: u8,
        total_segments: u8,
    }
    impl FlirSegment {
        fn is_valid_flir(&self) -> bool {
            &self.signature == b"FLIR\0"
        }
    }

    let mut flir_segments = vec![];
    let mut num_copied = 0;
    let mut total_len = 0;

    for segment in image.segments_by_marker(markers::APP1) {
        let contents = segment.contents();
        if contents.len() < 8 {
            continue;
        }
        let segment: FlirSegment = deserialize(&contents)?;
        if !segment.is_valid_flir() {
            continue;
        }

        let FlirSegment {
            current_segment,
            total_segments,
            ..
        } = segment;
        let total_segments = 1 + total_segments as usize;

        match flir_segments.len() {
            0 => {
                flir_segments.resize(total_segments, vec![]);
            }
            l if l != total_segments => {
                bail!(
                    "inconsistent count of total FLIR segments: {} != {}",
                    l,
                    total_segments
                );
            }
            l if l <= current_segment as usize => {
                bail!(
                    "FLIR segment idx out of bounds: {} >= {}",
                    current_segment,
                    l
                );
            }
            _ => {}
        }

        let curr_seg = &mut flir_segments[current_segment as usize];
        if curr_seg.len() > 0 {
            bail!("duplicate FLIR segment: idx = {}", current_segment);
        }
        curr_seg.extend_from_slice(&contents[8..]);
        num_copied += 1;
        total_len += curr_seg.len();
    }
    if num_copied != flir_segments.len() {
        bail!(
            "expected {} FLIR segments, found only {}",
            flir_segments.len(),
            num_copied
        );
    }
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
    pub fn try_parse_raw_data(&self, segment: &[u8]) -> Result<Option<Array2<u16>>> {
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
                raw_data.push(deserialize_with_endian(is_le, &mut raw_data_slice)?);
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
