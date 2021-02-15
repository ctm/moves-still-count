mod move_scraper;

use {
    crate::move_scraper::{DatedMoves, Month, MoveScraper, Year},
    anyhow::Result,
    chrono::NaiveDate,
    std::{
        env,
        fs::File,
        io::{ErrorKind, Read},
    },
    structopt::StructOpt,
};

fn main() -> Result<()> {
    let name = env::var("MOVESCOUNT_NAME")?;
    let password = env::var("MOVESCOUNT_PASSWORD")?;
    let opt = Opt::from_args();

    let mut dated_moves = saved_moves()?;
    let mut scraper = MoveScraper::new(&name, &password)?;
    // scraper.set_year_and_month(opt.year, opt.month)?;
    // let mut dated_moves = scraper.moves_from_page()?;
    // scraper.advance_month()?;
    // dated_moves.extend(scraper.moves_from_page()?.into_iter());

    // let start = NaiveDate::from_ymd(2020, 4, 15);
    // let stop = NaiveDate::from_ymd(2020, 6, 15);
    // let dated_moves = scraper.moves_for_range(&(start..stop));

    let start = NaiveDate::from_ymd(opt.year.into(), opt.month.into(), 1);
    let scraped_moves = scraper.moves_for_range(&(start..))?;
    move_scraper::merge(&mut dated_moves, &scraped_moves);
    save_moves(&dated_moves)?;
    // eprintln!("dated_moves: {:#?}", dated_moves);
    let mut been_here = false;
    for dmove in dated_moves {
        scraper.save_html_moves(&dmove)?;
        scraper.export_moves(&dmove, &mut been_here, opt.export)?;
    }
    Ok(())
}

const SAVED_MOVES_FILENAME: &str = "saved_moves.json";

fn saved_moves() -> Result<Vec<DatedMoves>> {
    match File::open(SAVED_MOVES_FILENAME) {
        Ok(mut file) => {
            let mut data = String::new();
            file.read_to_string(&mut data)?;
            serde_json::from_str(&data).map_err(|e| e.into())
        }
        Err(e) if e.kind() == ErrorKind::NotFound => Ok(Vec::new()),
        Err(other) => Err(other.into()),
    }
}

fn save_moves(moves: &[DatedMoves]) -> Result<()> {
    let mut file = File::create(SAVED_MOVES_FILENAME)?;
    serde_json::to_writer(&mut file, moves).map_err(|e| e.into())
}

#[derive(StructOpt, Debug)]
#[structopt()]
pub struct Opt {
    #[structopt(short = "e", long)]
    pub export: bool,
    #[structopt(short = "y", long, default_value)]
    year: Year,
    #[structopt(short = "m", long, default_value)]
    month: Month,
}
