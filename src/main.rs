// Command line interface for the Beeminder REST API.
// Allows shell scripting of Beeminder for usecases such as custom goal reminders or adding data
// points through cron.
use anyhow::bail;
use anyhow::Result;
use chrono::naive::NaiveDateTime;
use chrono::serde::ts_seconds;
use chrono::DateTime;
use chrono::Utc;
use log::info;
use regex::Regex;
use serde::Deserialize;
use serde::Serialize;
use std::io::prelude::*;
use structopt::StructOpt;

#[derive(StructOpt)]
enum GoalCommand {
    #[structopt(name = "list", about = "List all goals of a user")]
    List,
    #[structopt(name = "info", about = "List information about a goal")]
    Info { goal: String },
}

#[derive(StructOpt)]
enum DatapointCommand {
    #[structopt(name = "list", about = "List datapoints of a goal")]
    List { goal: String },
    #[structopt(name = "create", about = "Create a datapoint from CLI flags")]
    Create {
        goal: String,
        #[structopt(short = "-v", long)]
        value: f64,
        #[structopt(short = "-t", long)]
        timestamp: Option<u64>,
        #[structopt(short = "-d", long)]
        daystamp: Option<String>,
        #[structopt(short = "-c", long)]
        comment: Option<String>,
        #[structopt(short = "-i", long)]
        request_id: Option<String>,
    },
    #[structopt(name = "put", about = "Create datapoints from STDIN")]
    Put { goal: String },
}

#[derive(StructOpt)]
enum Command {
    #[structopt(name = "user", about = "Relates to Beeminder user")]
    User,
    #[structopt(name = "goal", about = "Relates to goals of a Beeminder user")]
    Goal(GoalCommand),
    #[structopt(
        name = "datapoint",
        about = "Related to datapoints of a Beeminder user"
    )]
    Datapoint(DatapointCommand),
}

#[derive(StructOpt)]
#[structopt(about = "CLI for Beeminder REST API")]
struct Cli {
    #[structopt(
        short,
        long,
        about = "Beeminder API token. If unset uses BEEMINDER_API_TOKEN environment variable"
    )]
    token: Option<String>,
    #[structopt(
        short,
        long,
        about = "Beeminder username. If unset uses BEEMINDER_USERNAME environment variable"
    )]
    user: Option<String>,
    #[structopt(subcommand)]
    cmd: Command,
}

static APP_USER_AGENT: &str = concat!(env!("CARGO_PKG_NAME"), "/", env!("CARGO_PKG_VERSION"));

struct BeeminderUrl {
    base: String,
    token: String,
}

impl BeeminderUrl {
    fn new(user: &str, token: &str) -> BeeminderUrl {
        BeeminderUrl {
            base: format!("https://www.beeminder.com/api/v1/users/{}", user),
            token: token.to_string(),
        }
    }

    fn build(&self, part: &str) -> String {
        format!("{}{}?auth_token={}", self.base, part, self.token)
    }
}

fn get_token(args: &Cli) -> Option<String> {
    if let Some(token) = &args.token {
        Some(token.to_string())
    } else {
        match std::env::var("BEEMINDER_API_TOKEN") {
            Ok(val) => Some(val),
            _ => None,
        }
    }
}

fn get_user(args: &Cli) -> Option<String> {
    if let Some(user) = &args.user {
        Some(user.to_string())
    } else {
        match std::env::var("BEEMINDER_USER") {
            Ok(val) => Some(val),
            _ => None,
        }
    }
}

#[derive(Serialize, Deserialize, Debug)]
struct User {
    created_at: u64,
    goals: Vec<String>,
    id: String,
    timezone: String,
    updated_at: u64,
    urgency_load: u64,
    username: String,
}

#[derive(Serialize, Deserialize, Debug)]
struct Goal {
    slug: String,
    #[serde(with = "ts_seconds")]
    updated_at: DateTime<Utc>,
    title: String,
    goal_type: String,
    yaxis: String,
    #[serde(with = "ts_seconds")]
    goaldate: DateTime<Utc>,
    #[serde(with = "ts_seconds")]
    losedate: DateTime<Utc>,
    safesum: String,
}

#[derive(Serialize, Deserialize, Debug)]
struct Datapoint {
    id: String,
    #[serde(with = "ts_seconds")]
    timestamp: DateTime<Utc>,
    daystamp: String,
    value: f64,
    comment: Option<String>,
    #[serde(with = "ts_seconds")]
    updated_at: DateTime<Utc>,
    requestid: Option<String>,
}

fn datapoints_from_stdin(goal: &str) -> Result<Vec<Datapoint>> {
    let bytes = std::io::stdin().bytes().collect::<Result<Vec<_>, _>>()?;
    let input = std::str::from_utf8(&bytes)?;
    let regex =
        Regex::new(r"(\d{4}-\d{2}-\d{2} \d{2}:\d{2}:\d{2}) ([0-9]+(\.[0-9]+)?)( '([^']+)')?")
            .unwrap();
    let mut points = vec![];
    for line in input.lines() {
        match regex.captures(line) {
            Some(captures) => {
                let daystamp = NaiveDateTime::parse_from_str(
                    captures.get(1).unwrap().as_str(),
                    "%Y-%m-%d %H:%M:%S",
                )?;
                points.push(Datapoint {
                    id: format!("beeminder {} {}", goal, daystamp),
                    timestamp: DateTime::from_utc(daystamp, Utc),
                    daystamp: daystamp.format("%Y%m%d").to_string(),
                    value: captures.get(2).unwrap().as_str().parse::<f64>().unwrap(),
                    comment: captures.get(5).map(|m| m.as_str().to_string()),
                    requestid: None,
                    updated_at: Utc::now(),
                });
            }
            None => bail!("Invalid line: {}", line),
        }
    }
    Ok(points)
}

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::init();
    let args = Cli::from_args();
    let url = BeeminderUrl::new(
        &get_user(&args).expect("Expected Beeminder user"),
        &get_token(&args).expect("Expected Beeminder API token"),
    );
    let builder = reqwest::ClientBuilder::new();
    let client = builder.user_agent(APP_USER_AGENT).build()?;
    match args.cmd {
        Command::User => {
            info!("Retrieving user data from Beeminder");
            let response = client.get(url.build(".json")).send().await?;
            let user: User = response.json().await?;
            println!("{}", serde_json::to_string(&user).unwrap());
        }
        Command::Goal(cmd) => match cmd {
            GoalCommand::List => {
                info!("Retrieving goals from Beeminder");
                let response = client
                    .get(url.build(&format!("/goals.json")))
                    .send()
                    .await?;
                let goal: Vec<Goal> = response.json().await?;
                println!("{}", serde_json::to_string(&goal).unwrap());
            }
            GoalCommand::Info { goal } => {
                info!("Retrieving goal data from Beeminder");
                let response = client
                    .get(url.build(&format!("/goals/{}.json", goal)))
                    .send()
                    .await?;
                let goal: Goal = response.json().await?;
                println!("{}", serde_json::to_string(&goal).unwrap());
            }
        },
        Command::Datapoint(cmd) => match cmd {
            DatapointCommand::List { goal } => {
                info!("Retrieving datapoint data from Beeminder");
                let response = client
                    .get(url.build(&format!("/goals/{}/datapoints.json", goal)))
                    .send()
                    .await?;
                let datapoints: Vec<Datapoint> = response.json().await?;
                println!("{}", serde_json::to_string(&datapoints).unwrap());
            }
            DatapointCommand::Create {
                goal,
                value,
                timestamp,
                daystamp,
                comment,
                request_id,
            } => {
                info!("Creating new data point");
                let mut params = vec![];
                params.push(("value", value.to_string()));
                if let Some(t) = timestamp {
                    params.push(("timestamp", t.to_string()));
                }
                if let Some(d) = daystamp {
                    params.push(("daystamp", d.to_string()));
                }
                if let Some(c) = comment {
                    params.push(("comment", c));
                }
                if let Some(r) = request_id {
                    params.push(("requestid", r));
                }
                client
                    .post(url.build(&format!("/goals/{}/datapoints.json", goal)))
                    .form(&params)
                    .header(reqwest::header::CONTENT_TYPE, "application/json")
                    .header(reqwest::header::ACCEPT, "application/json")
                    .send()
                    .await?;
            }
            DatapointCommand::Put { goal } => {
                info!("Creating several datapoints");
                let points = datapoints_from_stdin(&goal)?;
                client
                    .post(url.build(&format!("/goals/{}/datapoints/create_all.json", goal)))
                    .form(&[("datapoints", serde_json::to_string(&points).unwrap())])
                    .header(reqwest::header::CONTENT_TYPE, "application/json")
                    .header(reqwest::header::ACCEPT, "application/json")
                    .send()
                    .await?;
            }
        },
    }
    Ok(())
}
