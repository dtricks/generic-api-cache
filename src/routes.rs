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
    let out;
    // let mut out = format!(
    //     "headers: {:#?}, body_bytes: {:?}, method: {:?}, path: {:?}",
    //     headers, body, method, &path
    // );
    let path_string = format!("{}", path.as_str());
    let cached: Option<String> =
        tokio::task::spawn_blocking(move || cache.get(path.as_str())).await??;
    if let Some(cached) = cached {
        log::info!("Accessing from cache");
        out = cached;
    } else {
        // do client request here
        let uri: hyper::Uri =
            format!("{}{}", CONFIG.get_str("target_api")?, path_string).parse()?;
        dbg!(&uri);
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
            dbg!(&new_headers);
        }
        let req = req.body(body.into())?;
        let res = client.request(req).await?;
        let body = res.into_body();
        let text = hyper::body::to_bytes(body).await?;
        out = format!("{}", std::str::from_utf8(&text)?);
        let out_cache = out.clone();
        tokio::task::spawn_blocking(move || {
            cache2.set(
                &path_string,
                &out_cache,
                CONFIG.get("cache_time_in_seconds").unwrap_or(600),
            )
        })
        .await??;
    }
    log::debug!("{}", &out);
    Ok(warp::http::Response::new(out))
}
