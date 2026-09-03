use anyhow::Result;
use std::sync::Arc;

use seslogin::app;
use seslogin::graphql;
use seslogin::jwt;
use seslogin::mockdb;
use seslogin::mockmail;
use seslogin::mockqueue;

#[tokio::main]
async fn main() -> Result<()> {
    let key = jwt::Key::new("schema-export", None, None)?;
    let db = mockdb::Handler::new();
    // Nothing here dispatches anything; the mocks keep schema export off the network.
    let app = Arc::new(app::new(
        db,
        key,
        0,
        mockqueue::Handler::new(),
        mockmail::Handler::new(),
    ));
    let webauthn = Arc::new(app::build_webauthn()?);
    let schema = graphql::build_schema(app, webauthn);

    print!("{}", schema.sdl());

    Ok(())
}
