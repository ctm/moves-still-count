use {
    self::sample_rec::{setter_for, SampleRec, TrkPt},
    anyhow::Result,
    chrono::NaiveDateTime,
    std::{
        convert::TryInto,
        fs::File,
        io::{BufWriter, Read},
    },
    xml::{
        common::XmlVersion,
        name::OwnedName,
        reader::{EventReader, XmlEvent},
        EmitterConfig,
    },
};

mod sample_rec;

type EventWriter = xml::writer::EventWriter<BufWriter<File>>;

pub(crate) struct MoveConverter<R: Read> {
    reader: EventReader<R>,
}

type WriteResult = std::result::Result<(), xml::writer::Error>;

impl<R: Read> MoveConverter<R> {
    pub(crate) fn new(reader: R) -> Self {
        Self {
            reader: EventReader::new(reader),
        }
    }

    pub(crate) fn convert(self) -> Result<()> {
        use XmlEvent::*;

        let mut sample: SampleRec = Default::default();
        let mut f = None;
        let mut dumped = false;
        let mut cadence_seen = false;
        let mut writer = None;
        for elem in self.reader {
            match elem? {
                StartElement {
                    name: OwnedName { local_name, .. },
                    ..
                } => f = setter_for(local_name),
                EndElement {
                    name: OwnedName { local_name, .. },
                    ..
                } if local_name == "Sample" && sample.is_periodic() => {
                    if writer.is_none() {
                        writer = Some(Self::writer_for_sample(&sample)?);
                    }
                    if cadence_seen || !dumped {
                        sample.dump(writer.as_mut().unwrap())?;
                        dumped = true;
                    }
                }
                Characters(value) => {
                    if let Some(setter) = f {
                        setter(&mut sample, value);
                        if !cadence_seen && sample.has_cadence() {
                            cadence_seen = true;
                        }
                    }
                }
                _ => {}
            }
        }
        Self::write_postlude(writer.expect("no writer"))?;
        Ok(())
    }

    fn writer_for_sample(sample: &SampleRec) -> Result<EventWriter> {
        // To generate the proper name, we need to suck up the
        // characters from a DateTime tag, since that's the only
        // source of the local time.  Everything else is UTC.
        let local_time: NaiveDateTime = sample.local_time.parse()?;
        let filename = local_time
            .format("Move_%Y_%m_%d_%H_%M_%S_Running.gpx")
            .to_string();
        let writer = BufWriter::new(File::create(&filename)?);
        let mut writer = EmitterConfig::new()
            .line_separator("\r\n")
            .perform_indent(true)
            .create_writer(writer);
        writer.write_prelude()?;
        Ok(writer)
    }

    fn write_postlude(mut writer: EventWriter) -> Result<()> {
        Self::close_trkseg(&mut writer)?;
        Self::close_trk(&mut writer)?;
        Self::close_gpx(&mut writer)?;
        Ok(())
    }

    fn close_trkseg(writer: &mut EventWriter) -> WriteResult {
        writer.end_element()
    }

    fn close_trk(writer: &mut EventWriter) -> WriteResult {
        writer.end_element()
    }

    fn close_gpx(writer: &mut EventWriter) -> WriteResult {
        writer.end_element()
    }
}

trait DumpToGpx {
    fn dump(&self, writer: &mut EventWriter) -> Result<()>;
}

impl DumpToGpx for SampleRec {
    fn dump(&self, writer: &mut EventWriter) -> Result<()> {
        match TryInto::<TrkPt>::try_into(self) {
            Ok(pt) => pt.dump(writer),
            Err(e) => {
                eprintln!("dropping {:?}: {:?}", self, e);
                Ok(())
            }
        }
    }
}

trait EventWriterExt {
    fn write_prelude(&mut self) -> Result<()>;
    fn write_document_declaration(&mut self) -> WriteResult;
    fn open_gpx(&mut self) -> WriteResult;
    fn open_trk(&mut self) -> WriteResult;
    fn name(&mut self, name: &str) -> WriteResult;
    fn open_trkseg(&mut self) -> WriteResult;
    fn start_element(&mut self, element: &str) -> WriteResult;
    fn end_element(&mut self) -> WriteResult;
    fn dump_element<V: ToString>(&mut self, element: &str, value: V) -> WriteResult;
}

impl EventWriterExt for EventWriter {
    fn write_prelude(&mut self) -> Result<()> {
        self.write_document_declaration()?;
        self.open_gpx()?;
        self.open_trk()?;
        self.name("Move")?;
        self.open_trkseg()?;
        Ok(())
    }

    fn write_document_declaration(&mut self) -> WriteResult {
        self.write(xml::writer::XmlEvent::StartDocument {
            standalone: Some(false),
            version: XmlVersion::Version10,
            encoding: Some("utf-8"),
        })
    }

    fn open_gpx(&mut self) -> WriteResult {
        let gpx = xml::writer::XmlEvent::start_element("gpx")
            .attr("version", "1.1")
            .attr("creator", "Movescount - http://www.movescount.com")
            .ns("xsi", "http://www.w3.org/2001/XMLSchema-instance")
            .attr("xsi:schemaLocation", "http://www.topografix.com/GPX/1/1 http://www.topografix.com/GPX/1/1/gpx.xsd http://www.cluetrust.com/XML/GPXDATA/1/0 http://www.cluetrust.com/Schemas/gpxdata10.xsd http://www.garmin.com/xmlschemas/TrackPointExtension/v1 http://www.garmin.com/xmlschemas/TrackPointExtensionv1.xsd")
            .ns("gpxdata", "http://www.cluetrust.com/XML/GPXDATA/1/0")
            .ns("gpxtpx", "http://www.garmin.com/xmlschemas/TrackPointExtension/v1")
            .default_ns("http://www.topografix.com/GPX/1/1");
        self.write(gpx)
    }

    fn open_trk(&mut self) -> WriteResult {
        self.start_element("trk")
    }

    fn name(&mut self, name: &str) -> WriteResult {
        self.start_element("name")?;
        self.write(xml::writer::XmlEvent::characters(name))?;
        self.end_element()
    }

    fn open_trkseg(&mut self) -> WriteResult {
        self.start_element("trkseg")
    }

    fn start_element(&mut self, element: &str) -> WriteResult {
        self.write(xml::writer::XmlEvent::start_element(element))
    }

    fn end_element(&mut self) -> WriteResult {
        self.write(xml::writer::XmlEvent::end_element())
    }

    fn dump_element<V: ToString>(&mut self, element: &str, value: V) -> WriteResult {
        let value = value.to_string();
        self.start_element(element)?;
        self.write(xml::writer::XmlEvent::characters(&value))?;
        self.end_element()
    }
}

impl DumpToGpx for TrkPt {
    fn dump(&self, writer: &mut EventWriter) -> Result<()> {
        let lat = self.latitude();
        let lon = self.longitude();
        let trkpt = xml::writer::XmlEvent::start_element("trkpt")
            .attr("lat", &lat)
            .attr("lon", &lon);
        writer.write(trkpt)?;
        writer.dump_element("ele", self.altitude_m)?;
        writer.dump_element("time", self.time())?;
        writer.start_element("extensions")?;
        writer.start_element("gpxtpx:TrackPointExtension")?;
        if let Some(hr_bpm) = self.hr_bpm {
            writer.dump_element("gpxtpx:hr", hr_bpm)?;
        }
        writer.end_element(/* gpxtpx */)?;

        let has_cadence;
        match self.cadence_ffm {
            None => has_cadence = false,
            Some(ffm) => {
                has_cadence = true;
                writer.dump_element("gpxdata:cadence", ffm)?;
            }
        }

        writer.dump_element("gpxdata:temp", self.temperature_c)?;

        if has_cadence {
            writer.dump_element("gpxdata:distance", self.distance_m)?;
            writer.dump_element("gpxdata:altitude", self.altitude_m)?;
        }

        writer.dump_element("gpxdata:seaLevelPressure", self.sea_level_pressure_millibar)?;
        writer.dump_element("gpxdata:speed", self.speed_mps)?;
        writer.dump_element("gpxdata:verticalSpeed", self.vertical_speed_mps)?;
        writer.end_element(/* extensions */)?;
        writer.end_element().map_err(|e| e.into())
    }
}
