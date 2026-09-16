use clap::{Parser, Subcommand};
use sqlx::postgres::PgPoolOptions;

#[derive(Parser)]
#[command(name = "stellar-pay-cli")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    // Calls into `stellar_pay_api::run_server` so startup logic is not
    // duplicated between the `api` binary and the CLI.
    Serve,
    Migrate,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenvy::dotenv().ok();
    tracing_subscriber::fmt::init();

    let cli = Cli::parse();

    match cli.command {
        Command::Serve => {
            stellar_pay_api::run_server().await?;
        }
        Command::Migrate => {
            run_migrations().await?;
        }
    }

    return Ok(());
}

async fn run_migrations() -> anyhow::Result<()> {
    let database_url = std::env::var("DATABASE_URL")?;

    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&database_url)
        .await?;

    sqlx::migrate!("../../migrations").run(&pool).await?;

    tracing::info!("migrations applied");

    return Ok(());
}
