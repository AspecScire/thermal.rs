//! Library to process thermal images from FLIR cameras.
//!
//! This crate provides two functionalities:
//!
//! 1. Compute [temperature] from raw sensor values and
//! ambient parameters (typically stored as metadata in the
//! image). The code is a port of the [Thermimage R
//! library] and its [python port][read_thermal.py].
//!
//! 2. [Parse parameters](flir) and raw sensor values from
//! image metadata. This is a port of relevant parts of
//! [ExifTool] by Phil Harvey and other authors. Also
//! provides functionality to [parse the JSON](exif) output
//! of `exiftool`
//!
//! [read_thermal.py]: //github.com/Nervengift/read_thermal.py/blob/master/flir_image_extractor.py
//! [Thermimage R library]: //github.com/gtatters/Thermimage/blob/master/R/raw2temp.R
//! [ExifTool]: //exiftool.org
#[macro_use]
mod parse;

pub mod exif;
pub mod flir;
pub mod temperature;

pub mod args;
pub mod image;
pub mod stats;
