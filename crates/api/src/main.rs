use stellar_pay_api::run_server;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenvy::dotenv().ok();
    tracing_subscriber::fmt::init();

    run_server().await?;

    return Ok(());
}
