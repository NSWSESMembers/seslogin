//! The media Lambda: serves `GET /media/{asset_id}/{variant}` (see [`seslogin::media`])
//! behind CloudFront's `/media/*`. It needs only the asset bucket (`ASSET_BUCKET`) and
//! the server secret that signs media URLs (`JWT_SECRET`): no DynamoDB.
//!
//! Like the rest of the media code it never answers 403 or 404: CloudFront rewrites both
//! to `/index.html` with a 200.

use std::sync::Arc;

use lambda_http::{Body, Error, Request, Response, run, service_fn, tracing};
use seslogin::server_secret::ServerSecret;
use seslogin::{clock, media, s3};

struct State {
    storage: s3::Handler,
    secret: ServerSecret,
}

fn reply(r: media::MediaResponse, head: bool) -> Result<Response<Body>, Error> {
    let body = if head { Vec::new() } else { r.body };
    Ok(Response::builder()
        .status(r.status)
        .header("Content-Type", r.content_type)
        .header("Cache-Control", r.cache_control)
        .body(Body::Binary(body))?)
}

async fn handle(state: &State, request: Request) -> Result<Response<Body>, Error> {
    let head = request.method() == http::Method::HEAD;
    if !head && request.method() != http::Method::GET {
        return Ok(Response::builder()
            .status(405)
            .header("Allow", "GET, HEAD")
            .header("Cache-Control", media::NO_STORE)
            .body(Body::Empty)?);
    }
    let Some((asset_id, variant)) = media::parse_path(request.uri().path()) else {
        // Not 404: see the module docs.
        return reply(
            media::MediaResponse {
                status: 400,
                content_type: "text/plain; charset=utf-8",
                cache_control: media::NO_STORE,
                body: b"Bad request".to_vec(),
            },
            head,
        );
    };
    let response = media::serve(
        &state.storage,
        asset_id,
        variant,
        request.uri().query().unwrap_or(""),
        clock::now_sec(),
        &state.secret,
    )
    .await;
    tracing::info!(status = response.status, asset_id, variant, "media");
    reply(response, head)
}

#[tokio::main]
async fn main() -> Result<(), Error> {
    tracing::init_default_subscriber();
    let secret = std::env::var("JWT_SECRET").expect("JWT_SECRET must be set");
    let state = Arc::new(State {
        storage: s3::Handler::from_env().await,
        secret: ServerSecret::new(&secret),
    });
    run(service_fn(|request| {
        let state = state.clone();
        async move { handle(&state, request).await }
    }))
    .await
}
