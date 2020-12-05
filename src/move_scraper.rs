use {
    anyhow::{anyhow, bail, Result},
    chrono::{
        format::{DelayedFormat, StrftimeItems},
        Datelike, Local, NaiveDate, NaiveDateTime, Timelike,
    },
    serde::{Deserialize, Serialize},
    std::{
        collections::HashMap,
        convert::{TryFrom, TryInto},
        fmt::{self, Display, Formatter},
        fs::File,
        io::Write,
        num::NonZeroU8,
        ops::{Bound, RangeBounds},
        path::Path,
        str::FromStr,
        time::Duration,
    },
    thirtyfour_sync::{
        prelude::*,
        By::{Css, LinkText},
        WebDriver,
    },
};

pub(crate) struct MoveScraper {
    driver: WebDriver,
    year_month: Option<(Year, Month)>,
    current_move: Option<Move>,
}

impl MoveScraper {
    pub(crate) fn new(name: &str, password: &str) -> WebDriverResult<Self> {
        let caps = DesiredCapabilities::firefox();
        let driver = WebDriver::new("http://localhost:4444", &caps)?;
        driver.get("https://www.movescount.com/auth?redirect_uri=%2flatestmove")?;

        driver.find_element(Css("#splEmail"))?.send_keys(name)?;
        driver
            .find_element(Css("#splPassword"))?
            .send_keys(password)?;
        driver.find_element(Css("#splLoginButton"))?.click()?;

        // This is just to delay us until we know the authorization has worked.
        // There's nothing special about Tools
        driver.find_element(LinkText("Tools"))?;
        Ok(Self {
            driver,
            year_month: None,
            current_move: None,
        })
    }

    pub(crate) fn set_year_and_month(&mut self, year: Year, month: Month) -> Result<()> {
        if self.year_month != Some((year, month)) {
            let url = format!(
                "https://www.movescount.com/summary#calendar-month={}-{}&moves=",
                year, month
            );
            self.driver.get(&url)?;
            self.current_move = None;

            // This hack is because various elements are dynamically
            // reloaded in a way that we can't really look for all the
            // calendar dates until things have settled with getting a
            // stale element reference
            self.update_year_month()?;
            self.year_month = Some((year, month));
        }
        Ok(())
    }

    pub(crate) fn moves_from_page(&self) -> Result<Vec<DatedMoves>> {
        if self.year_month.is_none() {
            bail!("No year and month");
        }

        let timeouts = self.driver.get_timeouts()?;
        self.driver
            .set_implicit_wait_timeout(Duration::from_millis(0))?;
        let result = (|| {
            let mut ones_seen = 0;

            // We do the filter_map first, because it's more efficient, but
            // the code will read better if I break some of this into
            // a helper method
            Ok(self
                .driver
                .find_elements(Css("div.calendar-day"))?
                .into_iter()
                .filter_map(|day| {
                    // We need to look at the date even if there are
                    // no a.calendar-stack items, because we count the
                    // first days of the month so we can determine
                    // whether a given day is associated with the
                    // previous, current or next month.
                    let date = match self.date_from_calendar_day(&mut ones_seen, &day) {
                        Err(e) => return Some(Err(e)),
                        Ok(d) => d,
                    };

                    day.find_elements(Css("a.calendar-stack")).map_or_else(
                        |err| Some(Err(err.into())),
                        |elems| {
                            if elems.is_empty() {
                                None
                            } else {
                                Some(
                                    elems
                                        .into_iter()
                                        .map(|e| e.try_into())
                                        .collect::<Result<Vec<_>>>()
                                        .map_or_else(Err, |moves| Ok(DatedMoves { date, moves })),
                                )
                            }
                        },
                    )
                })
                .collect::<Result<Vec<_>>>()?)
        })();
        self.driver.set_timeouts(timeouts)?;
        result
    }

    pub(crate) fn moves_for_range<T: RangeBounds<NaiveDate>>(
        &mut self,
        range: &T,
    ) -> Result<Vec<DatedMoves>> {
        // NOTE: Currently we disallow unbounded starts, but we could
        // do something like look at the total number of moves and
        // then look at all the months going backward until we find
        // all the moves and then know where the first move is.  This
        // is, however, throw-away code, so that seems silly.
        use Bound::*;

        let start = match range.start_bound() {
            Unbounded => bail!("Unbounded starts not supported"),
            Included(start) | Excluded(start) => start,
        };
        let year_month_top = match range.end_bound() {
            Unbounded => {
                let today = Local::today();
                NaiveDate::from_ymd(today.year(), today.month(), 1)
            }
            Included(start) | Excluded(start) => {
                NaiveDate::from_ymd(start.year(), start.month(), 1)
            }
        };
        let mut results = Vec::new();
        let mut year;
        let mut month;
        self.set_year_and_month(start.year().try_into()?, start.month().try_into()?)?;
        while {
            let (y, m) = self.year_month.unwrap();
            year = y.into();
            month = m.into();
            let current_year_month = NaiveDate::from_ymd(year, month, 1);
            current_year_month <= year_month_top
        } {
            results.extend(self.moves_from_page()?.into_iter().filter(|m| {
                let date = &m.date;
                range.contains(date) && date.month() == month && date.year() == year
            }));
            self.advance_month()?;
        }
        Ok(results)
    }

    pub(crate) fn advance_month(&mut self) -> Result<()> {
        if self.year_month.is_none() {
            bail!("No year and month");
        }
        self.driver.find_element(Css(".icon-154"))?.click()?;
        let (year, month) = self.next_month();
        self.year_month = Some((year.try_into()?, month.try_into()?));
        Ok(())
    }

    pub(crate) fn save_html_moves(&mut self, dmove: &DatedMoves) -> Result<()> {
        let prefix = dmove.prefix();
        for to_save in &dmove.moves {
            self.save_html(*to_save, &prefix)?;
        }
        Ok(())
    }

    // This method serves two functions.  It makes sure year_month is up to
    // date and it also waits long enough that we can access various dynamic
    // bits of the DOM that get changed in a way that prevents us from simply
    // using find_element or find_elements.
    fn update_year_month(&mut self) -> Result<()> {
        let e = self.driver.find_element(Css("#calendarDate"))?;
        let mut retries_left = 30;
        let mut month_year;
        while {
            month_year = e.text()?;
            month_year.is_empty() && retries_left > 0
        } {
            retries_left -= 1;
            std::thread::sleep(Duration::from_millis(500));
        }
        if month_year.is_empty() {
            bail!("Month and year never appeared");
        }
        let month_year_day = format!("{} 01", e.text()?);
        let date = NaiveDate::parse_from_str(&month_year_day, "%B %Y %d")?;
        self.year_month = Some((date.year().try_into()?, date.month().try_into()?));
        Ok(())
    }

    fn save_html(&mut self, to_save: Move, prefix: &DelayedFormat<StrftimeItems>) -> Result<()> {
        let filename = format!("{}{}.html", prefix, to_save.0);

        if !Path::new(&filename).exists() {
            self.goto_move(to_save)?;
            let html = self.driver.find_element(Css("html"))?.outer_html()?;
            File::create(&filename)?.write_all(html.as_bytes())?;
        }
        Ok(())
    }

    fn goto_move(&mut self, to_goto: Move) -> Result<()> {
        if self.current_move != Some(to_goto) {
            let url = format!("https://www.movescount.com/moves/move{}", to_goto);
            self.driver.get(&url)?;
            self.current_move = Some(to_goto);
            self.update_year_month()?;
        }
        Ok(())
    }

    pub(crate) fn export_moves(
        &mut self,
        dmove: &DatedMoves,
        been_here: &mut bool,
        really_export: bool,
    ) -> Result<()> {
        let prefix = dmove.prefix();
        let n = dmove.moves.len();
        for to_export in &dmove.moves {
            self.export(*to_export, &prefix, n, been_here, really_export)?;
        }
        Ok(())
    }

    fn export(
        &mut self,
        to_export: Move,
        prefix: &DelayedFormat<StrftimeItems>,
        n: usize,
        been_here: &mut bool,
        really_export: bool,
    ) -> Result<()> {
        let fileglob = format!("{}??_??_??*.gpx", prefix);

        let paths = glob::glob(&fileglob)?
            .map(|r| r.map_err(|e| e.into()))
            .collect::<Result<Vec<_>>>()?;
        let count = paths.len();

        if count > n {
            bail!(
                "Found {} gpx files, but only know of {} moves, prefix: {}",
                count,
                n,
                prefix
            );
        }

        if n > count {
            if !really_export {
                eprintln!("Not trying to export {}:{}", prefix, to_export);
                return Ok(());
            }
            self.goto_move(to_export)?;
            if self.gpx_file_is_already_present(prefix)? {
                return Ok(());
            }
            let tools = self.driver.find_element(LinkText("Tools"))?;
            self.driver
                .action_chain()
                .move_to_element_center(&tools)
                .perform()?;
            std::thread::sleep(Duration::from_secs(5)); // New, only used after
                                                        // I scraped 2,228 moves.  Not sure that this helps, but it appears
                                                        // that my longer moves haven't downloaded previously.
            self.driver
                .find_element(LinkText("Export as GPX"))?
                .click()?;
            let duration = if *been_here {
                4 // I used to use 2, but I'm getting more delay now
            } else {
                *been_here = true;
                10
            };
            std::thread::sleep(Duration::from_secs(duration));
        }
        Ok(())
    }

    // This only works after we've visited a particular move, because
    // we have to extract the hour and minute from the page in order
    // for our glob to be sufficiently narrow to prevent false
    // positives.  FWIW, there's no way we can get the starting
    // second, so we still have to glob, which means that we'll run
    // into trouble if there are two legitimate moves that both start
    // on the same minute.  I don't think I'll ever have such moves
    // though.
    fn gpx_file_is_already_present(
        &mut self,
        prefix: &DelayedFormat<StrftimeItems>,
    ) -> Result<bool> {
        let start = NaiveDateTime::parse_from_str(
            &self.driver.find_element(Css(".feed-content-top"))?.text()?,
            "%m/%d/%Y %H:%M",
        )?;
        let fileglob = format!(
            "{}{:02}_{:02}_??*.gpx",
            prefix,
            start.hour(),
            start.minute()
        );
        match glob::glob(&fileglob)?.next() {
            None => Ok(false),
            Some(Ok(_)) => Ok(true),
            Some(Err(e)) => Err(e.into()),
        }
    }

    // ones_seen is used to deal with calendar dates from the previous
    // or the next month that are on this page.  Until we actually see
    // the first of the month, we're on the previous month.  Once
    // we've seen two first of months, the second is in the next
    // month.
    fn date_from_calendar_day(
        &self,
        ones_seen: &mut u8,
        calendar_day: &WebElement,
    ) -> Result<NaiveDate> {
        let day = calendar_day
            .find_element(Css(".calendar-day-number"))?
            .text()?
            .parse()?;
        if day == 1 {
            *ones_seen += 1;
        }
        let (year, month) = match *ones_seen {
            0 => self.previous_month(),
            1 => self.this_month(),
            2 => self.next_month(),
            n => bail!("Seen {} first days of the month", n),
        };
        // NaiveDate::from_ymd_opt(year, month, day).ok_or_else(|| anyhow!("impossible date, year: {}, month: {}, day: {}, *ones_seen: {}", year, month, day, *ones_seen))
        NaiveDate::from_ymd_opt(year, month, day).ok_or_else(|| {
            std::thread::sleep(Duration::from_secs(500));
            anyhow!(
                "impossible date, year: {}, month: {}, day: {}, *ones_seen: {}",
                year,
                month,
                day,
                *ones_seen
            )
        })
    }

    fn previous_month(&self) -> (i32, u32) {
        let (year, month) = self.this_month();
        if month == Month::MIN_MONTH as u32 {
            (year - 1, Month::MAX_MONTH as u32)
        } else {
            (year, month - 1)
        }
    }

    fn this_month(&self) -> (i32, u32) {
        let (year, month) = self.year_month.unwrap();
        (year.into(), month.into())
    }

    fn next_month(&self) -> (i32, u32) {
        let (year, month) = self.this_month();
        if month == Month::MAX_MONTH as u32 {
            (year + 1, Month::MIN_MONTH as u32)
        } else {
            (year, month + 1)
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub(crate) struct Move(u32);

impl Move {}

impl Display for Move {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl TryFrom<WebElement<'_>> for Move {
    type Error = anyhow::Error;

    fn try_from(e: WebElement) -> Result<Self> {
        e.get_attribute("data-id")?.map_or_else(
            || bail!("no data-id"),
            |id| {
                id.strip_prefix("move-")
                    .ok_or_else(|| anyhow!("couldn't find prefix"))
                    .and_then(|id| Ok(Self(id.parse()?)))
            },
        )
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub(crate) struct DatedMoves {
    date: NaiveDate,
    moves: Vec<Move>,
}

impl DatedMoves {
    fn prefix(&self) -> DelayedFormat<StrftimeItems> {
        self.date.format("Move_%Y_%m_%d_")
    }
}

// Making range limited Year and Month was an experiment.  Turns out
// it's lots of boilerplate.  There's probably a better way to do it.

#[derive(Debug)]
pub(crate) enum ParseYearError {
    NotNumeric,
    OutOfRange,
}

impl Display for ParseYearError {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        use ParseYearError::*;

        match self {
            NotNumeric => write!(f, "must be numeric"),
            OutOfRange => write!(f, "must be >= {} and <= {}", Year::MIN_YEAR, Year::MAX_YEAR),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct Year(u8);

impl Year {
    const MIN_YEAR: u16 = 2010;
    const MAX_YEAR: u16 = 2021;

    fn new(value: u16) -> Option<Self> {
        if value < Self::MIN_YEAR || value > Self::MAX_YEAR {
            return None;
        }
        let u: Option<u8> = (value - Self::MIN_YEAR).try_into().ok();
        u.map(Self)
    }
}

impl Default for Year {
    fn default() -> Self {
        Self::new(2013).unwrap()
    }
}

impl Display for Year {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        (Into::<u8>::into(self.0) as u16 + Self::MIN_YEAR).fmt(f)
    }
}

impl FromStr for Year {
    type Err = ParseYearError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        use ParseYearError::*;

        s.parse()
            .map_err(|_| NotNumeric)
            .and_then(|year| Self::new(year).ok_or(OutOfRange))
    }
}

impl From<Year> for i32 {
    fn from(year: Year) -> Self {
        (year.0 as u16 + Year::MIN_YEAR).into()
    }
}

impl TryFrom<i32> for Year {
    type Error = anyhow::Error;

    fn try_from(value: i32) -> Result<Self> {
        Self::new(value.try_into()?).ok_or_else(|| anyhow!("year out of range"))
    }
}

#[derive(Debug)]
pub(crate) enum ParseMonthError {
    NotNumeric,
    OutOfRange,
}

impl Display for ParseMonthError {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        use ParseMonthError::*;

        match self {
            NotNumeric => write!(f, "must be numeric"),
            OutOfRange => write!(
                f,
                "must be >= {} and <= {}",
                Month::MIN_MONTH,
                Month::MAX_MONTH
            ),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct Month(NonZeroU8);

impl Month {
    const MIN_MONTH: u8 = 1;
    const MAX_MONTH: u8 = 12;

    fn new(value: u8) -> Option<Self> {
        if value < Self::MIN_MONTH || value > Self::MAX_MONTH {
            return None;
        }
        NonZeroU8::new(value).map(Self)
    }
}

impl Default for Month {
    fn default() -> Self {
        Self::new(11).unwrap()
    }
}

impl Display for Month {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl FromStr for Month {
    type Err = ParseMonthError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        use ParseMonthError::*;

        s.parse()
            .map_err(|_| NotNumeric)
            .and_then(|month| Self::new(month).ok_or(OutOfRange))
    }
}

impl From<Month> for u32 {
    fn from(month: Month) -> Self {
        month.0.get().into()
    }
}

impl TryFrom<u32> for Month {
    type Error = anyhow::Error;

    fn try_from(value: u32) -> Result<Self> {
        Self::new(value.try_into()?).ok_or_else(|| anyhow!("month out of range"))
    }
}

pub(crate) fn merge(dest: &mut Vec<DatedMoves>, src: &[DatedMoves]) {
    let mut h = HashMap::new();
    h.extend(dest.iter().map(|dm| (dm.date, dm.moves.clone())));
    for DatedMoves { date, moves } in src {
        let v = h.entry(*date).or_insert_with(Vec::new);
        for to_merge in moves {
            if !v.contains(to_merge) {
                v.push(*to_merge);
            }
        }
    }
    *dest = h
        .into_iter()
        .map(|(date, moves)| DatedMoves { date, moves })
        .collect::<Vec<_>>();
}
