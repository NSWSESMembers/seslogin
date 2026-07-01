use anyhow::{Result, anyhow};
use clap::{Parser, Subcommand};
use seslogin::jwt::{ExpirePolicy, Key};

#[derive(Parser, Debug)]
#[command(about = "Generate a JWT for a session or user")]
struct Cli {
    /// JWT secret (overrides JWT_SECRET env var).
    #[arg(long)]
    jwt_secret: Option<String>,

    /// Override JWT expiry in seconds.
    #[arg(long)]
    expire_s: Option<u64>,

    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand, Debug)]
enum Command {
    /// Generate a JWT for a session (default expiry: 14 days).
    Session {
        /// The session ID to embed in the JWT.
        session_id: String,
    },
    /// Generate a JWT for a user (default expiry: 1 hour).
    User {
        /// The user ID to embed in the JWT.
        user_id: String,
    },
}

fn main() -> Result<()> {
    seslogin::load_cli_env();

    let cli = Cli::parse();

    let secret = cli
        .jwt_secret
        .or_else(|| std::env::var("JWT_SECRET").ok())
        .ok_or_else(|| anyhow!("JWT_SECRET is required (flag or env var)"))?;

    println!("Using secret: {}", secret);

    let key = Key::new(&secret, None, None)?;

    let expire_policy = if let Some(s) = cli.expire_s {
        ExpirePolicy::TimeSec(s)
    } else {
        match &cli.command {
            Command::Session { .. } => ExpirePolicy::SessionDefault,
            Command::User { .. } => ExpirePolicy::UserDefault,
        }
    };

    let token = match &cli.command {
        Command::Session { session_id } => key.make_session_jwt(session_id, expire_policy)?,
        Command::User { user_id } => key.make_user_jwt(user_id, expire_policy)?,
    };

    println!("{}", token);

    Ok(())
}
