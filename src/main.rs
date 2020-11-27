use {
    anyhow::Result,
    fantoccini::{
        Client,
        Locator::{Css, LinkText},
    },
    std::env,
    structopt::StructOpt,
};

#[tokio::main]
async fn main() -> Result<()> {
    let name = env::var("MOVESCOUNT_NAME")?;
    let password = env::var("MOVESCOUNT_PASSWORD")?;
    let opt = Opt::from_args();

    let mut caps = serde_json::map::Map::new();

    let firefox_opts = if opt.display {
        serde_json::json!({ "args": [] })
    } else {
        serde_json::json!({ "args": ["--headless"] })
    };
    caps.insert("moz:firefoxOptions".to_string(), firefox_opts);

    let mut c = Client::with_capabilities("http://localhost:4444", caps).await?;
    c.goto("https://www.movescount.com/auth?redirect_uri=%2flatestmove")
        .await?;

    // Need to fill in name and password
    let mut email = c.find(Css("#splEmail")).await?;
    email.send_keys(&name).await?;

    let mut pass = c.find(Css("#splPassword")).await?;
    pass.send_keys(&password).await?;

    let button = c.find(Css("#splLoginButton")).await?;
    button.click().await?;

    let tools = c.wait_for_find(LinkText("Tools")).await?;
    tools.click().await?;

    let export = c.wait_for_find(LinkText("Export as GPX")).await?;
    export.click().await?;

    Ok(())
}

#[derive(StructOpt, Debug)]
#[structopt()]
pub struct Opt {
    /// See the webpage as results are gathered
    #[structopt(short = "d", long = "display")]
    pub display: bool,
}
