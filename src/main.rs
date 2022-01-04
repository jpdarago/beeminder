// Command line interface for the Beeminder REST API.
//
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
    #[structopt(name = "put", about = "Create datapoints from STDIN formatted input.")]
    Put { goal: String },
    #[structopt(name = "delete", about = "Delete datapoints")]
    Delete { goal: String, id: String },
}

#[derive(StructOpt)]
enum Command {
    #[structopt(name = "user", about = "Relates to a Beeminder user")]
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
        about = "Beeminder API token. If unset uses BEEMINDER_AUTH_TOKEN environment variable or Beeminder config file"
    )]
    auth_token: Option<String>,
    #[structopt(
        short,
        long,
        about = "Beeminder username. If unset uses BEEMINDER_USERNAME environment variable or Beeminder config file"
    )]
    username: Option<String>,
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

#[derive(Serialize, Deserialize, Debug)]
struct Auth {
    username: Option<String>,
    auth_token: Option<String>,
}

impl ::std::default::Default for Auth {
    fn default() -> Self {
        Self {
            username: None,
            auth_token: None,
        }
    }
}

impl Auth {
    // Load authentication information with the following order of preference, descending:
    //
    // - Command line argument (i.e. --username or --auth_token).
    // - Environment variable (i.e. BEEMINDER_USERNAME).
    // - Configuration file.
    fn load(args: &Cli) -> Result<Self> {
        let token = {
            if let Some(token) = &args.auth_token {
                Some(token.to_string())
            } else {
                std::env::var("BEEMINDER_AUTH_TOKEN").ok()
            }
        };
        let user = if let Some(user) = &args.username {
            Some(user.to_string())
        } else {
            std::env::var("BEEMINDER_USERNAME").ok()
        };
        let mut auth: Auth = confy::load("beeminder")?;
        if token.is_some() {
            auth.auth_token = token;
        }
        if user.is_some() {
            auth.username = user;
        }
        Ok(auth)
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
    let auth = Auth::load(&args)?;
    info!("Authentication information: {:?}", auth);
    let user = auth.username.expect("No Beeminder user provided.");
    let token = auth.auth_token.expect("No Beeminder token provided.");
    let url = BeeminderUrl::new(&user, &token);
    let builder = reqwest::ClientBuilder::new();
    let client = builder.user_agent(APP_USER_AGENT).build()?;
    match args.cmd {
        Command::User => {
            info!("Retrieving user data for {}", user);
            let response = client.get(url.build(".json")).send().await?;
            let user: User = response.json().await?;
            println!("{}", serde_json::to_string(&user).unwrap());
        }
        Command::Goal(cmd) => match cmd {
            GoalCommand::List => {
                info!("Retrieving goals for user {}", user);
                let response = client.get(url.build("/goals.json")).send().await?;
                let goal: Vec<Goal> = response.json().await?;
                println!("{}", serde_json::to_string(&goal).unwrap());
            }
            GoalCommand::Info { goal } => {
                info!("Retrieving goal data for goal {} user {}", user, goal);
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
                info!("Retrieving datapoint data for goal {} user {}", goal, user);
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
                info!("Creating new data point for goal {} user {}", goal, user);
                let mut params = vec![("value", value.to_string())];
                if let Some(t) = timestamp {
                    params.push(("timestamp", t.to_string()));
                }
                if let Some(d) = daystamp {
                    params.push(("daystamp", d));
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
                info!(
                    "Creating datapoints from standard input for goal {} user {}",
                    goal, user
                );
                let points = datapoints_from_stdin(&goal)?;
                client
                    .post(url.build(&format!("/goals/{}/datapoints/create_all.json", goal)))
                    .form(&[("datapoints", serde_json::to_string(&points).unwrap())])
                    .header(reqwest::header::CONTENT_TYPE, "application/json")
                    .header(reqwest::header::ACCEPT, "application/json")
                    .send()
                    .await?;
            }
            DatapointCommand::Delete { goal, id } => {
                info!("Deleting data point {} for goal {} user {}", id, goal, user);
                client
                    .delete(url.build(&format!("/goals/{}/datapoints/{}.json", goal, id)))
                    .send()
                    .await?;
            }
        },
    }
    Ok(())
}
