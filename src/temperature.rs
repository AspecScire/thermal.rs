//! Functions to compute temperature from raw sensor values.
//!
//! Ported from [read_thermal.py] which itself is ported from
//! [Thermimage R library].
//!
//! [read_thermal.py]: //github.com/Nervengift/read_thermal.py/blob/master/flir_image_extractor.py
//! [Thermimage R library]: //github.com/gtatters/Thermimage/blob/master/R/raw2temp.R

/// Parameters to compute temperatures from raw sensor
/// values.
///
/// This is typically read from the exif data of the image.
use serde_derive::*;

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "PascalCase")]
pub struct ThermalSettings {
    #[serde(
        rename = "RelativeHumidity",
        deserialize_with = "serde_helpers::float_with_suffix"
    )]
    relative_humidity_percentage: f64,

    emissivity: f64,
    #[serde(deserialize_with = "serde_helpers::float_with_suffix")]
    reflected_apparent_temperature: f64,

    #[serde(
        rename = "IRWindowTemperature",
        deserialize_with = "serde_helpers::float_with_suffix"
    )]
    ir_window_temperature: f64,
    #[serde(rename = "IRWindowTransmission")]
    ir_window_transmission: f64,

    planck_r1: f64,
    planck_b: f64,
    planck_f: f64,
    planck_o: f64,
    planck_r2: f64,

    #[serde(deserialize_with = "serde_helpers::float_with_suffix")]
    atmospheric_temperature: f64,
    #[serde(rename = "AtmosphericTransAlpha1")]
    atmospheric_transmission_alpha_1: f64,
    #[serde(rename = "AtmosphericTransAlpha2")]
    atmospheric_transmission_alpha_2: f64,
    #[serde(rename = "AtmosphericTransBeta1")]
    atmospheric_transmission_beta_1: f64,
    #[serde(rename = "AtmosphericTransBeta2")]
    atmospheric_transmission_beta_2: f64,
    #[serde(rename = "AtmosphericTransX")]
    atmospheric_transmission_x: f64,
}

const CELICIUS_OFFSET: f64 = 273.15;
impl ThermalSettings {
    // raw = PR1/(PR2*(exp(PB/(temp+273.15))-PF))-PO
    fn planck_temp_to_raw(&self, temp: f64) -> f64 {
        self.planck_r1
            / (self.planck_r2 * ((self.planck_b / (temp + CELICIUS_OFFSET)).exp() - self.planck_f))
            - self.planck_o
    }

    // inverse of above
    fn planck_raw_to_temp(&self, raw: f64) -> f64 {
        self.planck_b
            / (self.planck_r1 / (self.planck_r2 * (raw + self.planck_o)) + self.planck_f).ln()
            - CELICIUS_OFFSET
    }

    // tau1<-ATX*exp(-sqrt(OD/2)*(ATA1+ATB1*sqrt(h2o)))
    //  +(1-ATX)*exp(-sqrt(OD/2)*(ATA2+ATB2*sqrt(h2o)))
    fn atmospheric_affine1(&self, val: f64) -> f64 {
        self.atmospheric_transmission_alpha_1 +
            self.atmospheric_transmission_beta_1 * val
    }

    fn atmospheric_affine2(&self, val: f64) -> f64 {
        self.atmospheric_transmission_alpha_2 +
            self.atmospheric_transmission_beta_2 * val
    }

    fn atmospheric_interpolate(&self, val1: f64, val2: f64) -> f64 {
        self.atmospheric_transmission_x * val1 + (1. - self.atmospheric_transmission_x) * val2
    }

    pub fn raw_transform(&self, distance: f64) -> impl Fn(f64) -> f64 {
        // This is step to step port of the R code

        //   emiss.wind<-1-IRT
        let emiss_wind = 1. - self.ir_window_transmission;

        //   refl.wind<-0 # anti-reflective coating on window
        let refl_wind = 0.;


        // ############ transmission through the air
        //   h2o<-(RH/100)*exp(1.5587+0.06939*(ATemp)-0.00027816*(ATemp)^2+0.00000068455*(ATemp)^3)
        //   # converts relative humidity into water vapour pressure (I think in units mmHg)
        const ATMOSPHERIC_SERIES: [f64; 4] = [1.5587, 0.06939, -0.00027816, 0.00000068455];
        let h2o = (self.relative_humidity_percentage / 100.) *
            power_series_at(
                &ATMOSPHERIC_SERIES, self.atmospheric_temperature
            ).exp();

        let h2o_sqrt = h2o.sqrt();

        //   tau1<-ATX*exp(-sqrt(OD/2)*(ATA1+ATB1*sqrt(h2o)))+(1-ATX)*exp(-sqrt(OD/2)*(ATA2+ATB2*sqrt(h2o)))
        //   tau2<-ATX*exp(-sqrt(OD/2)*(ATA1+ATB1*sqrt(h2o)))+(1-ATX)*exp(-sqrt(OD/2)*(ATA2+ATB2*sqrt(h2o)))
        //   # transmission through atmosphere - equations from Minkina and Dudzik's Infrared Thermography Book
        //   # Note: for this script, we assume the thermal window is at the mid-point (OD/2) between the source
        //   # and the camera sensor
        let dist_factor = (distance as f64 / 2.).sqrt();

        let tau = self.atmospheric_interpolate(
            (-dist_factor * self.atmospheric_affine1(h2o_sqrt)).exp(),
            (-dist_factor * self.atmospheric_affine2(h2o_sqrt)).exp(),
        );

        //   raw.refl1<-PR1/(PR2*(exp(PB/(RTemp+273.15))-PF))-PO   # radiance reflecting off the object before the window
        //   raw.refl1.attn<-(1-E)/E*raw.refl1   # attn = the attenuated radiance (in raw units)
        let refl1 = self.planck_temp_to_raw(self.reflected_apparent_temperature);
        let refl1_attn = (1. - self.emissivity) / self.emissivity * refl1;

        //   raw.atm1<-PR1/(PR2*(exp(PB/(ATemp+273.15))-PF))-PO # radiance from the atmosphere (before the window)
        //   raw.atm1.attn<-(1-tau1)/E/tau1*raw.atm1 # attn = the attenuated radiance (in raw units)
        let atm1 = self.planck_temp_to_raw(self.atmospheric_temperature);
        let atm1_attn = (1. - tau) / tau / self.emissivity * atm1;

        //   raw.wind<-PR1/(PR2*(exp(PB/(IRWTemp+273.15))-PF))-PO
        //   raw.wind.attn<-emiss.wind/E/tau1/IRT*raw.wind
        let wind = self.planck_temp_to_raw(self.ir_window_temperature);
        let wind_attn = emiss_wind / self.emissivity / tau
            / self.ir_window_transmission * wind;

        //   raw.refl2<-PR1/(PR2*(exp(PB/(RTemp+273.15))-PF))-PO
        //   raw.refl2.attn<-refl.wind/E/tau1/IRT*raw.refl2
        let refl2 = self.planck_temp_to_raw(self.reflected_apparent_temperature);
        let refl2_attn = refl_wind / self.emissivity / tau
            / self.ir_window_transmission * refl2;

        //   raw.atm2<-PR1/(PR2*(exp(PB/(ATemp+273.15))-PF))-PO
        //   raw.atm2.attn<-(1-tau2)/E/tau1/IRT/tau2*raw.atm2
        let atm2 = self.planck_temp_to_raw(self.atmospheric_temperature);
        let atm2_attn = (1. - tau) / self.emissivity / tau
            / self.ir_window_transmission / tau * atm2;

        let coeffs = [
            - atm1_attn - atm2_attn
                - wind_attn
                - refl1_attn - refl2_attn,
            1. / self.emissivity / tau
                / self.ir_window_transmission / tau,
        ];

        move |raw| power_series_at(&coeffs, raw)

    }

    pub fn temperature_transform(&self, distance: f64) -> impl Fn(f64) -> f64 + '_ {
        let t = self.raw_transform(distance);
        move |raw| {
            let raw = t(raw);
            self.planck_raw_to_temp(raw)
        }
    }

    pub fn raw_to_temp(&self, distance: f64, raw: f64) -> f64 {
        self.temperature_transform(distance)(raw)
    }
}

#[inline]
fn power_series_at(coeffs: &[f64], x: f64) -> f64 {
    let mut pow = 1.;
    let mut sum = 0.;
    for coeff in coeffs.iter() {
        sum += pow * coeff;
        pow *= x;
    }
    sum
}

mod serde_helpers {
    use lazy_static::lazy_static;
    use regex::Regex;
    use serde::*;
    pub fn float_with_suffix<'de, D>(de: D) -> Result<f64, D::Error>
    where
        D: Deserializer<'de>,
    {
        lazy_static! {
            static ref RE: Regex = Regex::new(r"^\d*.\d*").unwrap();
        }

        use serde::de::Error;
        let str_rep = <String as Deserialize>::deserialize(de)?;
        let val = RE
            .find(&str_rep)
            .ok_or(Error::custom("unexpected format: must begin with float"))?
            .as_str()
            .parse()
            .map_err(Error::custom)?;

        Ok(val)
    }
}
