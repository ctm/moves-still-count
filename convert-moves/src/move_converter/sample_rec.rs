use {
    anyhow::{Context, Result},
    chrono::{DateTime, Utc},
    std::{
        convert::{TryFrom, TryInto},
        f32::consts::PI,
        num::TryFromIntError,
    },
};

type SampleRecSetter = fn(&mut SampleRec, String);

pub(crate) fn setter_for(name: String) -> Option<SampleRecSetter> {
    SETTERS_FOR_ELEMENTS
        .iter()
        .find(|(elem, _)| elem == &name)
        .map(|(_, setter)| *setter)
}

#[derive(Debug, Default)]
pub(crate) struct SampleRec {
    pub(crate) local_time: String, // Comes from Header, not Sample
    latitude_ster: String,
    longitude_ster: String,
    vertical_speed_mps: String,
    cadence_ffs: String,
    hr_bps: String,
    temperature_k: String,
    sea_level_pressure_pa: String,
    altitude_m: String,
    distance_m: String,
    speed_mps: String,
    elapsed_time_sec: String,
    sample_type: String,
    time_utc: String,
}

impl SampleRec {
    fn local_time(&mut self, value: String) {
        self.local_time = value;
    }

    fn latitude_ster(&mut self, value: String) {
        self.latitude_ster = value;
    }

    fn longitude_ster(&mut self, value: String) {
        self.longitude_ster = value;
    }

    fn vertical_speed_mps(&mut self, value: String) {
        self.vertical_speed_mps = value;
    }

    fn cadence_ffs(&mut self, value: String) {
        self.cadence_ffs = value;
    }

    fn hr_bps(&mut self, value: String) {
        self.hr_bps = value;
    }

    fn temperature_k(&mut self, value: String) {
        self.temperature_k = value;
    }

    fn sea_level_pressure_pa(&mut self, value: String) {
        self.sea_level_pressure_pa = value;
    }

    fn altitude_m(&mut self, value: String) {
        self.altitude_m = value;
    }

    fn distance_m(&mut self, value: String) {
        self.distance_m = value;
    }

    fn speed_mps(&mut self, value: String) {
        self.speed_mps = value;
    }

    fn elapsed_time_sec(&mut self, value: String) {
        self.elapsed_time_sec = value;
    }

    fn sample_type(&mut self, value: String) {
        self.sample_type = value;
    }

    fn time_utc(&mut self, value: String) {
        self.time_utc = value;
    }

    pub(crate) fn is_periodic(&self) -> bool {
        self.sample_type == "periodic"
    }

    pub(crate) fn has_cadence(&self) -> bool {
        !self.cadence_ffs.is_empty()
    }
}

static SETTERS_FOR_ELEMENTS: [(&str, SampleRecSetter); 15] = [
    ("DateTime", SampleRec::local_time),
    ("GPSAltitude", SampleRec::altitude_m),
    ("Latitude", SampleRec::latitude_ster),
    ("Longitude", SampleRec::longitude_ster),
    ("VerticalSpeed", SampleRec::vertical_speed_mps),
    ("Cadence", SampleRec::cadence_ffs),
    ("HR", SampleRec::hr_bps),
    ("Temperature", SampleRec::temperature_k),
    ("SeaLevelPressure", SampleRec::sea_level_pressure_pa),
    ("Altitude", SampleRec::altitude_m),
    ("Distance", SampleRec::distance_m),
    ("Speed", SampleRec::speed_mps),
    ("Time", SampleRec::elapsed_time_sec),
    ("SampleType", SampleRec::sample_type),
    ("UTC", SampleRec::time_utc),
];

#[derive(Debug)]
pub(crate) struct TrkPt {
    latitude_degrees: f32,
    longitude_degrees: f32,
    time_utc: DateTime<Utc>,
    pub(crate) hr_bpm: Option<u16>,
    pub(crate) cadence_ffm: Option<u16>,
    pub(crate) temperature_c: f32,
    pub(crate) distance_m: f32,
    pub(crate) altitude_m: f32,
    pub(crate) sea_level_pressure_millibar: u16,
    pub(crate) speed_mps: f32,
    pub(crate) vertical_speed_mps: f32,
}

impl TrkPt {
    pub(crate) fn latitude(&self) -> String {
        format!("{}", self.latitude_degrees)
    }

    pub(crate) fn longitude(&self) -> String {
        format!("{}", self.longitude_degrees)
    }

    pub(crate) fn time(&self) -> String {
        self.time_utc
            .to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
    }
}

impl TryFrom<&SampleRec> for TrkPt {
    type Error = anyhow::Error;

    fn try_from(value: &SampleRec) -> Result<Self, Self::Error> {
        let cadence_ffm = if value.cadence_ffs.is_empty() {
            None
        } else {
            Some(ffm_from_ffs(value.cadence_ffs.parse().context("cadence")?)?)
        };
        let hr_bpm = if value.hr_bps.is_empty() {
            None
        } else {
            Some(bpm_from_bps(value.hr_bps.parse().context("hr")?)?)
        };
        Ok(TrkPt {
            latitude_degrees: degrees_from_ster(value.latitude_ster.parse().context("latitude")?),
            longitude_degrees: degrees_from_ster(
                value.longitude_ster.parse().context("longitude")?,
            ),
            time_utc: value.time_utc.parse()?,
            hr_bpm,
            cadence_ffm,
            temperature_c: c_from_k(value.temperature_k.parse().context("temperature")?),
            distance_m: value.distance_m.parse().context("distance")?,
            altitude_m: value.altitude_m.parse().context("altitude")?,
            sea_level_pressure_millibar: millibar_from_pa(
                value
                    .sea_level_pressure_pa
                    .parse()
                    .context("sea level pressure")?,
            )?,
            speed_mps: value.speed_mps.parse().context("speed")?,
            vertical_speed_mps: value.vertical_speed_mps.parse().context("vertical speed")?,
        })
    }
}

fn degrees_from_ster(ster: f32) -> f32 {
    ster * 180.0 / PI
}

fn seconds_from_minutes(minutes: f32) -> std::result::Result<u16, TryFromIntError> {
    ((minutes * 60.0).round() as i32).try_into()
}

fn bpm_from_bps(bps: f32) -> std::result::Result<u16, TryFromIntError> {
    seconds_from_minutes(bps)
}

fn ffm_from_ffs(ffs: f32) -> std::result::Result<u16, TryFromIntError> {
    seconds_from_minutes(ffs)
}

fn c_from_k(k: f32) -> f32 {
    k - 273.16
}

fn millibar_from_pa(pa: f32) -> std::result::Result<u16, TryFromIntError> {
    ((pa / 100.0).round() as i32).try_into()
}
