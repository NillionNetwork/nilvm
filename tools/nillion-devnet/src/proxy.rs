use axum::{
    body::Body,
    extract::{Request, State},
    response::{IntoResponse, Response},
    routing::any,
    Router,
};
use hyper::{StatusCode, Uri};
use hyper_util::{client::legacy::connect::HttpConnector, rt::TokioExecutor};
use std::net::SocketAddr;
use tokio::net::TcpListener;

type Client = hyper_util::client::legacy::Client<HttpConnector, Body>;

#[derive(Clone)]
struct ClientState {
    client: Client,
    nilchain_endpoint: String,
}

pub struct NilchainProxy;

impl NilchainProxy {
    pub async fn run(listen_endpoint: SocketAddr, nilchain_endpoint: String) -> anyhow::Result<()> {
        let client =
            hyper_util::client::legacy::Client::<(), ()>::builder(TokioExecutor::new()).build(HttpConnector::new());
        let state = ClientState { client, nilchain_endpoint };
        let app = Router::new().route("/", any(Self::handler)).with_state(state);
        let listener = TcpListener::bind(listen_endpoint).await?;
        tokio::spawn(async move {
            let _ = axum::serve(listener, app).await;
        });
        Ok(())
    }

    async fn handler(state: State<ClientState>, mut req: Request) -> Result<Response, StatusCode> {
        let State(ClientState { client, nilchain_endpoint }) = state;
        let path = req.uri().path();
        let path_query = req.uri().path_and_query().map(|v| v.as_str()).unwrap_or(path);

        let uri = format!("{nilchain_endpoint}{path_query}");

        *req.uri_mut() = Uri::try_from(uri).unwrap();

        // Forward the request and append the CORS header.
        let mut response = client.request(req).await.map_err(|_| StatusCode::BAD_REQUEST)?.into_response();

        let allow_origins = "*".parse().map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
        response.headers_mut().append("Access-Control-Allow-Origin", allow_origins);

        let allow_methods = "POST".parse().map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
        response.headers_mut().append("Access-Control-Allow-Methods", allow_methods);

        let allow_headers = "content-type".parse().map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
        response.headers_mut().append("Access-Control-Allow-Headers", allow_headers);

        Ok(response)
    }
}
