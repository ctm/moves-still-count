// Super hack-and-slash code to give me a GPX file to upload to Strava
// or to use with my own tools.

use {
    self::move_converter::MoveConverter,
    anyhow::{Context, Result},
    std::{fs::File, io::BufReader, path::PathBuf},
    structopt::StructOpt,
};

mod move_converter;

#[derive(StructOpt)]
/// Convert .sml files created by MovesLink to GPX files suitable for Strava
///
/// Now that Suunto has decommissioned MovesCount, my Ambit 3 (and Ambit 2)
/// watches are much less useful, unless I use Suunto's app, which I resent
/// being forced to use.  So, this app is able to convert the various files
/// found in ~/Library/Application Support/Suunto/Moveslink2 to a GPX file
/// that can be uploaded to Strava.
///
/// I also have another hacky program that extracts some statistics from my
/// interval training, and the GPX file that this creates is compatible with
/// that app, as well.
struct Opt {
    #[structopt(parse(from_os_str))]
    files: Vec<PathBuf>,
}

fn main() -> Result<()> {
    let opt = Opt::from_args();

    for file in &opt.files {
        let input = File::open(file).with_context(|| format!("Failed to open {:?}", file))?;
        let converter = MoveConverter::new(BufReader::new(input));
        converter.convert()?;
    }
    Ok(())
}
