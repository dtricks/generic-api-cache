use color_eyre::eyre::Result;
use config::Config;
use r2d2_memcache::MemcacheConnectionManager;
use warp::Filter;

#[macro_use]
extern crate lazy_static;

pub mod logger;
pub mod routes;

lazy_static! {
    pub static ref CONFIG: Config = {
        let mut config = config::Config::default();
        config
            .merge(config::File::with_name("Config"))
            .unwrap()
            .merge(config::Environment::with_prefix("APP"))
            .unwrap();
        config
    };
    pub static ref POOL: r2d2::Pool<MemcacheConnectionManager> = {
        let memcached_connspec = format!("{}", CONFIG.get_str("memcached_conn_url").unwrap());
        let memcached_manager = MemcacheConnectionManager::new(memcached_connspec);
        r2d2::Pool::builder()
            .max_size(CONFIG.get("memcached_pool_size").unwrap_or(4))
            .build(memcached_manager)
            .expect("Failed to build cache pool")
    };
}

#[tokio::main(flavor = "multi_thread", worker_threads = 4)]
async fn main() -> Result<()> {
    color_eyre::install()?;
    logger::init()?;
    let bind = format!(
        "{}:{}",
        CONFIG.get_str("address")?,
        CONFIG.get::<u16>("port")?
    );
    log::info!("Starting server at: {}", &bind);

    let routes = warp::any()
        .and(warp::header::headers_cloned())
        .and(warp::body::bytes())
        .and(warp::method())
        .and(warp::filters::path::full())
        .and_then(|headers, body, method, path| async move {
            let out = routes::process(headers, body, method, path).await?;
            Ok::<_, warp::Rejection>(out)
        });

    warp::serve(routes)
        .run(bind.parse::<std::net::SocketAddr>()?)
        .await;

    Ok(())
}
