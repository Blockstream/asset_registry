use std::net;
use std::path::PathBuf;

use bitcoin_hashes::hex::FromHex;
use elements::AssetId;
use hyper::rt::{Future, Stream};
use hyper::service::service_fn;
use hyper::{header, Body, Method, Request, Response, Server, StatusCode};
use serde_json::Value;
use std::sync::Arc;

#[cfg(feature = "cli")]
use structopt::StructOpt;

use crate::asset::Asset;
use crate::chain::ChainQuery;
use crate::errors::{join_err, Result, ResultExt};
use crate::registry::Registry;
use crate::util::serde_from_base64;

#[derive(Debug)]
#[cfg_attr(feature = "cli", derive(StructOpt))]
pub struct Config {
    #[cfg_attr(
        feature = "cli",
        structopt(
            short,
            long,
            parse(from_occurrences),
            help = "Increase verbosity (up to 3)"
        )
    )]
    verbose: usize,

    #[cfg_attr(
        feature = "cli",
        structopt(short, long, help = "http server listen address (host:port)")
    )]
    addr: net::SocketAddr,

    #[cfg_attr(
        feature = "cli",
        structopt(short, long = "db-path", help = "Path to database directory")
    )]
    db_path: PathBuf,

    #[cfg_attr(
        feature = "cli",
        structopt(
            short,
            long = "hook-cmd",
            help = "Hook script to run after every registry update"
        )
    )]
    hook_cmd: Option<String>,

    #[cfg_attr(
        feature = "cli",
        structopt(
            short,
            long = "esplora-url",
            help = "url for querying chain state using the esplora api"
        )
    )]
    esplora_url: String,
}

//type ResponseFuture = Box<Future<Item = Response<Body>, Error = hyper::Error> + Send>;

pub fn start_server(config: Config) -> Result<()> {
    info!("Web server config: {:?}", config);

    stderrlog::new().verbosity(config.verbose + 2).init().ok();

    let chain = ChainQuery::new(config.esplora_url);
    let registry = Arc::new(Registry::new(&config.db_path, chain, config.hook_cmd));

    let make_service = move || {
        let registry = Arc::clone(&registry);

        service_fn(move |req: Request<Body>| {
            let registry = Arc::clone(&registry);
            let method = req.method().clone();
            let uri = req.uri().clone();

            info!("processing {} {}", method, uri);

            Box::new(req.into_body().concat2().and_then(move |body| {
                Ok(match handle_req(method, uri, body, &registry) {
                    Ok(resp) => {
                        info!("replying with {:?}", resp);

                        Response::builder()
                            .status(resp.status())
                            .header(header::CONTENT_TYPE, resp.content_type())
                            .body(resp.body())
                            .unwrap()
                    }

                    Err(err) => {
                        warn!("error processing request: {:?}", err);

                        #[cfg(not(feature = "dev"))]
                        let body = join_err(&err);
                        #[cfg(feature = "dev")]
                        let body = format!("{:#?}", err);

                        Response::builder()
                            .status(StatusCode::BAD_REQUEST)
                            .body(Body::from(body))
                            .unwrap()
                    }
                })
            }))
        })
    };

    let server = Server::bind(&config.addr).serve(make_service);

    info!("Starting web server on {:?}", config.addr);
    hyper::rt::run(server.map_err(|e| warn!("server error: {:?}", e)));

    Ok(())
}

#[derive(Debug)]
enum Resp {
    Json(StatusCode, Value),
    Plain(StatusCode, String),
}

impl Resp {
    fn json<T>(code: StatusCode, value: T) -> Resp
    where
        T: serde::ser::Serialize,
    {
        Resp::Json(code, serde_json::to_value(value).unwrap())
    }
    fn plain(code: StatusCode, message: &str) -> Resp {
        Resp::Plain(code, message.into())
    }
    fn body(&self) -> Body {
        Body::from(match self {
            Resp::Plain(_, message) => message.into(),
            Resp::Json(_, value) => serde_json::to_string(value).unwrap(),
        })
    }
    fn content_type(&self) -> &'static str {
        match self {
            Resp::Plain(..) => "text/plain",
            Resp::Json(..) => "application/json",
        }
    }
    fn status(&self) -> StatusCode {
        match self {
            Resp::Plain(status, _) => *status,
            Resp::Json(status, _) => *status,
        }
    }
}

fn handle_req(
    method: Method,
    uri: hyper::Uri,
    body: hyper::Chunk,
    registry: &Registry,
) -> Result<Resp> {
    match (method, uri.path()) {
        (Method::POST, "/") => handle_update(body, registry),
        (Method::GET, path) => handle_get(&path[1..], registry),
        (Method::DELETE, path) => handle_delete(&path[1..], body, registry),

        _ => Ok(Resp::plain(StatusCode::NOT_FOUND, "Not Found")),
    }
}

fn handle_get(asset_id: &str, registry: &Registry) -> Result<Resp> {
    let asset_id = AssetId::from_hex(asset_id)?;

    Ok(match registry.load(&asset_id)? {
        Some(asset) => Resp::json(StatusCode::OK, asset),
        None => Resp::plain(StatusCode::NOT_FOUND, "Not Found"),
    })
}

fn handle_update(body: hyper::Chunk, registry: &Registry) -> Result<Resp> {
    let body = String::from_utf8(body.to_vec())?;

    let asset = Asset::from_request(
        serde_json::from_str(&body).context("failed parsing json request")?,
        registry.chain(),
    )?;

    debug!("write asset: {:?}", asset);

    registry.write(&asset)?;

    Ok(Resp::json(StatusCode::CREATED, &asset))
}

fn handle_delete(asset_id: &str, body: hyper::Chunk, registry: &Registry) -> Result<Resp> {
    let asset_id = AssetId::from_hex(asset_id)?;
    let asset = match registry.load(&asset_id)? {
        None => return Ok(Resp::plain(StatusCode::NOT_FOUND, "Not found")),
        Some(asset) => asset,
    };

    let body = String::from_utf8(body.to_vec())?;
    let request: DeletionRequest =
        serde_json::from_str(&body).context("failed parsing json request")?;

    registry.delete(&asset, &request.signature)?;

    Ok(Resp::plain(StatusCode::OK, "Asset deleted"))
}

#[derive(Deserialize)]
struct DeletionRequest {
    #[serde(deserialize_with = "serde_from_base64")]
    signature: Vec<u8>,
}

// needs to be run with --test-threads 1
#[cfg(test)]
mod tests {
    use super::*;
    use crate::{asset::Asset, chain, client::Client, entity, errors::OptionExt};
    use bitcoin_hashes::hex::ToHex;
    use std::{thread, time::Duration};

    lazy_static! {
        static ref CLIENT: Client = Client::new("http://localhost:49013".parse().unwrap());
    }

    fn spawn_test_server() {
        let config = Config {
            verbose: 1,
            hook_cmd: None,
            addr: "127.0.0.1:49013".parse().unwrap(),
            esplora_url: "http://localhost:58713".to_string(),
            db_path: std::env::temp_dir()
                .join(format!("asset-registry-testdb-{}", std::process::id())),
        };

        std::fs::create_dir_all(&config.db_path).unwrap();

        thread::spawn(|| start_server(config).unwrap());
    }

    #[test]
    fn test0_init() {
        stderrlog::new().verbosity(3).init().ok();

        entity::tests::spawn_mock_verifier_server();
        chain::tests::spawn_mock_esplora_server();

        spawn_test_server();

        thread::sleep(Duration::from_millis(250));
    }

    #[test]
    fn test1_write_commited() -> Result<()> {
        let asset = CLIENT.register(&serde_json::from_str(r#"{
            "asset_id":"837de7388f6a17261ddb8757ec4b64e01137bc7cf13e9f0ec653bdc0cafe44a0",
            "contract":{
                "entity":{"domain":"test.dev"},
                "issuer_pubkey":"03ed9530a9ae5aacdc377e3c9cfbf03a4b21c6af5fa45e2df73a52edb8ee2fe70f",
                "name":"Bar Coin",
                "ticker":"BAR",
                "version":0
            },
            "issuance_txin":{
                "txid":"4f8cf0a4d3fc20a718bbd010991b1db4101548d473e4d7894ac25dc40a57d899",
                "vin":0
            }
        }"#)?)?;

        assert_eq!(asset.name(), "Bar Coin");

        Ok(())
    }
    /*
    #[test]
    fn test2_write_signed() -> Result<()> {
        CLIENT.register(&serde_json::from_str(r#"{
            "asset_id": "9a51761132b7399d34819c2c5d03af71794ff3aa0f78a434ddf20605545c86f2",
            "issuance_txin": {"txid":"77f21099c47646b30a9978a1a39acf658f6eb9bd68f677d23f132c587bb93836", "vin":0},
            "issuance_prevout":{"txid":"8e818b4561de8c731db7cd7a3b67784d525f96ecc7b564b82d8a01cab390b2d4","vout":1},
            "contract": {"issuer_pubkey":"026be637f97bc191c27522577bd6fe284b54404321652fcc4eb62aa0f4cfd6d172"},
            "name": "Foo Coin",
            "ticker": "FOO",
            "precision": 3,
            "entity": { "domain": "test.dev" },
            "signature": "IAbn0kr44f8+HJI/qpNaXvU48b/L9mBZUli197Okg5BVYXin3xA1ilbxAvHZ00BL/0+3URIuVtAeqkl7WxWmuhY="
        }"#)?)?;

        Ok(())
    }

    #[test]
    #[should_panic(expected = "register should fail")]
    fn test3_write_invalid_sig() {
        CLIENT.register(&serde_json::from_str(r#"{
            "asset_id": "9a51761132b7399d34819c2c5d03af71794ff3aa0f78a434ddf20605545c86f2",
            "issuance_txin": {"txid":"77f21099c47646b30a9978a1a39acf658f6eb9bd68f677d23f132c587bb93836", "vin":0},
            "issuance_prevout":{"txid":"8e818b4561de8c731db7cd7a3b67784d525f96ecc7b564b82d8a01cab390b2d4","vout":1},
            "contract": {"issuer_pubkey":"026be637f97bc191c27522577bd6fe284b54404321652fcc4eb62aa0f4cfd6d172"},
            "name": "Foo Coin",
            "ticker": "FOX",
            "precision": 3,
            "entity": { "domain": "test.dev" },
            "signature": "IAbn0kr44f8+HJI/qpNaXvU48b/L9mBZUli197Okg5BVYXin3xA1ilbxAvHZ00BL/0+3URIuVtAeqkl7WxWmuhY="
        }"#).unwrap())
        .expect("register should fail")
    }
    */

    #[test]
    fn test4_get() -> Result<()> {
        let asset: Asset = CLIENT
            .get(&AssetId::from_hex(
                "837de7388f6a17261ddb8757ec4b64e01137bc7cf13e9f0ec653bdc0cafe44a0",
            )?)?
            .or_err("registered asset not found")?;

        debug!("asset: {:?}", asset);

        assert_eq!(
            asset.id().to_hex(),
            "837de7388f6a17261ddb8757ec4b64e01137bc7cf13e9f0ec653bdc0cafe44a0",
        );
        assert_eq!(asset.name(), "Bar Coin");
        Ok(())
    }
}
