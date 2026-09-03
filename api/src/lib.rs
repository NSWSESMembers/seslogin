pub mod activity_summary;
pub mod app;
pub mod auth;
pub mod client_info;
pub mod clock;
pub mod db;
pub mod dynamodb;
pub mod environment;
pub mod expire;
pub mod graphql;
pub mod jwt;
pub mod location_sync;
pub mod mail;
pub mod member_sync;
pub mod mockdb;
pub mod mockmail;
pub mod mockqueue;
pub mod nitc_export;
pub mod nonce;
pub mod period_link;
pub mod queue;
pub mod request_metrics;
pub mod server;
pub mod ses_api;
pub mod sesmail;
pub mod session_key;
pub mod sqs;
pub mod telemetry;
pub mod text_table;
pub mod turnstile;

/// Load local `.env`/`.env.secret` for CLI / dev binaries. The Lambda binaries
/// don't call this. AWS profile defaulting is handled natively by
/// [`aws_config_loader`], not by mutating the environment.
pub fn load_cli_env() {
    dotenvy::from_filename(".env").ok();
    dotenvy::from_filename(".env.secret").ok();
}

/// AWS SDK config loader with the project's default profile applied. Defaults to
/// the new account's `seslogin` SSO profile for local/CLI use, but only when
/// `AWS_PROFILE` is unset *and* no credentials are already present in the
/// environment — so an explicit `AWS_PROFILE`, the Lambda execution role, and
/// CI's OIDC credentials (all of which set `AWS_ACCESS_KEY_ID`) take precedence.
/// Callers `.load().await`, optionally adding region/credential overrides first.
pub fn aws_config_loader() -> aws_config::ConfigLoader {
    let mut loader = aws_config::defaults(aws_config::BehaviorVersion::latest());
    if std::env::var_os("AWS_PROFILE").is_none() && std::env::var_os("AWS_ACCESS_KEY_ID").is_none()
    {
        loader = loader.profile_name("seslogin");
    }
    loader
}
