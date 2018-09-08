extern crate pretty_env_logger;
#[macro_use]
extern crate log;
extern crate warp;
extern crate tokio;
extern crate tokio_io;
#[macro_use]
extern crate nom;
extern crate bytes;
extern crate time;
extern crate serde;
#[macro_use] extern crate serde_derive;
#[macro_use] extern crate clap;

mod metrics;
mod statsd;

use warp::Filter;
use warp::http::{Response,StatusCode};
use std::sync::{Arc,Mutex};
use std::fmt::Write;
use std::net::SocketAddr;
use clap::{App, Arg};

fn main() {
    pretty_env_logger::init();
    let matches = App::new("Grad")
        .about(
            "Aggregate, store, query and visualize your metrics, all in one tool",
        )
        .version(crate_version!())
        .arg(
            Arg::with_name("config")
                .short("c")
                .long("config")
                .value_name("DIR")
                .default_value("./conf")
                .help("Set a custom config directory")
                .takes_value(true),
        )
        .arg(
            Arg::with_name("host")
                .long("host")
                .value_name("HOST")
                .default_value("127.0.0.1")
                .help("Set host to listen on")
                .takes_value(true),
        )
        .arg(
            Arg::with_name("port")
                .short("p")
                .long("port")
                .value_name("PORT")
                .default_value("3000")
                .help("Set a custom port to listen on")
                .takes_value(true),
        )
        .arg(
            Arg::with_name("udp_port")
                .short("u")
                .long("udp-port")
                .value_name("UDP_PORT")
                .default_value("8125")
                .help("Set a custom port to listen for metrics on")
                .takes_value(true),
        )
        .get_matches();

    let port = matches.value_of("port").unwrap();
    let host = matches.value_of("host").unwrap();
    let udp_port = matches.value_of("udp_port").unwrap();
    let conf_dir = matches.value_of("config").unwrap();

    let server: SocketAddr = [host, port].join(":").parse().expect(
        "Unable to parse given server information",
    );

    let udp_server: SocketAddr = [host, udp_port].join(":").parse().expect(
        "Unable to parse given server information",
    );

    let metrics = Arc::new(Mutex::new(metrics::Metrics::new()));
    let _handle = statsd::start(metrics.clone(), udp_server.clone());

    let root = warp::index().map(|| {
        include_str!("../assets/index.html")
    });

    let metrics_filter = warp::any().map(move || metrics.clone());
    let data = warp::post2().and(warp::path("data"))
      .and(warp::body::json())
      .map(|body:metrics::Query| { info!("data: {:?}", body); body})
      .and(metrics_filter.clone())
      .and_then(send_data);

    let dashboards = warp::path("dashboards").and(warp::fs::dir(conf_dir.to_string()));

    let series = warp::path("series").and(metrics_filter).and_then(send_series);

    let routes = warp::get2().and(root.or(dashboards).or(series)).or(data);

    warp::serve(routes).run((server));
}

fn send_test() -> Result<String, warp::Rejection> {
  info!("get /test returning 404");
  Err(warp::reject::not_found())
}

fn send_data(q: metrics::Query, metrics: Arc<Mutex<metrics::Metrics>>) -> Result<impl warp::Reply, warp::Rejection> {
  info!("send_data: {:?}", q.key);
  match (*metrics.lock().unwrap()).query(&q) {
    Some(data) => {
      info!("reply ok for {:?}: {:?}", q.key, data);
      Ok(warp::reply::json(&data))
    },
    None       => {
      error!("reply not found for {:?}", q.key);
      Ok(warp::reply::json(&metrics::QueryResult {
        timestamps: Vec::new(),
        values: Vec::new(),
      }))
      //we should be able to reject with a 404
      //Err(warp::reject::not_found())

      /*
      let response = Response::builder().status(StatusCode::NOT_FOUND).body(());
      response..map(map_err(|e| {
        error!("error building HTTP 404 response: {:?}", e);
        warp::reject::not_found()
      })
      */
    },
  }
}

fn send_series(metrics: Arc<Mutex<metrics::Metrics>>) -> Result<impl warp::Reply, warp::Rejection> {
  let mut s = String::new();
  let state = (*metrics.lock().unwrap()).state();
  write!(&mut s, "{} series, {} values\n", state.series.keys().count(), state.counter);
  for (key, (metric_type, subseries)) in state.series.iter() {
    for (tags, count) in subseries.iter() {
      write!(&mut s, "\n{}[{}]: {:?}", key, count, metric_type);
      for tag in tags.iter() {
        write!(&mut s, ", {}={}", tag.key, tag.value);
      }
    }
  }
  Ok(s)
}

