use crate::{CONFIG, POOL};
use bytes::Bytes;
// use color_eyre::eyre::Result;
use http::header::HeaderMap;
use http::method::Method;
use hyper::Client;
use warp::filters::path::FullPath;

// type CacheConn = web::Data<CachePool>;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum AppError {
    #[error("Cache Error")]
    DatabaseError(#[from] r2d2::Error),
    #[error("Join Error")]
    JoinError(#[from] tokio::task::JoinError),
    #[error("Memcache Error")]
    MemcacheError(#[from] memcache::MemcacheError),
    #[error("Config Error")]
    ConfigError(#[from] config::ConfigError),
    #[error("URI Error")]
    URIError(#[from] http::uri::InvalidUri),
    #[error("Hyper Client Error")]
    HyperError(#[from] hyper::Error),
    #[error("Hyper Client Http Error")]
    HyperHttpError(#[from] http::Error),
    #[error("Utf8 Error")]
    Utf8Error(#[from] std::str::Utf8Error),
    #[error("Openssl Error")]
    OpensslError(#[from] openssl::error::ErrorStack),
    #[error("Serde Error")]
    SerdeError(#[from] bincode::Error),
}

impl warp::reject::Reject for AppError {}

pub async fn process(
    headers: HeaderMap,
    body: Bytes,
    method: Method,
    path: FullPath,
) -> Result<warp::http::Response<String>, AppError> {
    let cache = POOL.get()?;
    let cache2 = POOL.get()?;
    let sr;
    let path_string = format!("{}", path.as_str());
    let cached: Option<Vec<u8>> =
        tokio::task::spawn_blocking(move || cache.get(path.as_str())).await??;

    if let Some(cached) = cached {
        log::info!("Accessing {:?} from cache", &path_string);
        sr = bincode::deserialize(&cached)?;
    } else {
        // do client request here
        let uri: hyper::Uri =
            format!("{}{}", CONFIG.get_str("target_api")?, path_string).parse()?;
        log::info!("Accessing {:?} from target API {:?}", &path_string, &uri);
        let connector = hyper_openssl::HttpsConnector::new()?;

        let client = Client::builder().build::<_, hyper::Body>(connector);
        let mut req = hyper::Request::builder().uri(uri).method(method);
        {
            let new_headers = req.headers_mut().unwrap();
            for (header_key, header_value) in headers {
                if let Some(header_name) = header_key {
                    // skip host header since we are proxying
                    if header_name == "host" {
                        continue;
                    }
                    new_headers.insert(header_name, header_value);
                }
            }
        }
        let req = req.body(body.into())?;
        let res = client.request(req).await?;
        let (parts, body) = res.into_parts();
        let text = hyper::body::to_bytes(body).await?;
        let out = format!("{}", std::str::from_utf8(&text)?);
        sr = SerializedResponse {
            body: out.clone(),
            headers: parts.headers,
        };
        let out_cache = bincode::serialize(&sr)?;
        tokio::task::spawn_blocking(move || {
            cache2.set(
                &path_string,
                out_cache.as_slice(),
                CONFIG.get("cache_time_in_seconds").unwrap_or(600),
            )
        })
        .await??;
    }
    let mut res = warp::http::Response::builder();
    {
        let new_headers = res.headers_mut().unwrap();
        for (header_key, header_value) in sr.headers {
            if let Some(header_name) = header_key {
                new_headers.insert(header_name, header_value);
            }
        }
    }
    Ok(res.body(sr.body)?)
}

use serde::Deserialize;
use serde::Serialize;

#[derive(Serialize, Deserialize)]
struct SerializedResponse {
    body: String,

    #[serde(with = "http_serde::header_map")]
    headers: HeaderMap,
}
