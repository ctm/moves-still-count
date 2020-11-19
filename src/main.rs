use {anyhow::Result, fantoccini::Client, structopt::StructOpt};

#[tokio::main]
async fn main() -> Result<()> {
    let opt = Opt::from_args();

    let mut caps = serde_json::map::Map::new();

    let firefox_opts = if opt.display {
        serde_json::json!({ "args": [] })
    } else {
        serde_json::json!({ "args": ["--headless"] })
    };
    caps.insert("moz:firefoxOptions".to_string(), firefox_opts);

    let mut c = Client::with_capabilities("http://localhost:4444", caps).await?;
    c.goto("http://movescount.com").await?;

    Ok(())
}

#[derive(StructOpt, Debug)]
#[structopt()]
pub struct Opt {
    /// See the webpage as results are gathered
    #[structopt(short = "d", long = "display")]
    pub display: bool,
}
