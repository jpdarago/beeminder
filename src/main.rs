use chrono::serde::ts_seconds;
use chrono::DateTime;
use chrono::Utc;
use log::info;
use serde::Deserialize;
use serde::Serialize;
use structopt::StructOpt;

#[derive(StructOpt)]
enum GoalCommand {
    #[structopt(name = "list")]
    List,
    #[structopt(name = "info")]
    Info { goal: String },
}

#[derive(StructOpt)]
enum DatapointCommand {
    #[structopt(name = "info")]
    List { goal: String },
}

#[derive(StructOpt)]
enum Command {
    #[structopt(name = "user")]
    User,
    #[structopt(name = "goal")]
    Goal(GoalCommand),
    #[structopt(name = "datapoint")]
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

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
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
            println!("{:?}", user);
        }
        Command::Goal(cmd) => match cmd {
            GoalCommand::List => {
                info!("Retrieving goal data from Beeminder");
                let response = client
                    .get(url.build(&format!("/goals.json")))
                    .send()
                    .await?;
                let goal: Vec<Goal> = response.json().await?;
                println!("{:?}", goal);
            }
            GoalCommand::Info { goal } => {
                info!("Retrieving goal data from Beeminder");
                let response = client
                    .get(url.build(&format!("/goals/{}.json", goal)))
                    .send()
                    .await?;
                let goal: Goal = response.json().await?;
                println!("{:?}", goal);
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
                println!("{:?}", datapoints);
            }
        },
    }
    Ok(())
}
