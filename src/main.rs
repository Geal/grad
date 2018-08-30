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

mod metrics;
mod statsd;

use warp::Filter;
use warp::http::{Response,StatusCode};
use std::sync::{Arc,Mutex};
use std::fmt::Write;

fn main() {
    pretty_env_logger::init();
    let metrics = Arc::new(Mutex::new(metrics::Metrics::new()));
    let _handle = statsd::start(metrics.clone());

    let root = warp::index().map(|| {
        include_str!("../assets/index.html")
    });

    let metrics_filter = warp::any().map(move || metrics.clone());
    let data = warp::post2().and(warp::path("data"))
      .and(warp::body::json())
      .map(|body:metrics::Query| { info!("data: {:?}", body); body})
      .and(metrics_filter.clone())
      .and_then(send_data);

    let dashboards = warp::path("dashboards").and(warp::fs::dir("./conf"));

    let series = warp::path("series").and(metrics_filter).and_then(send_series);

    let routes = warp::get2().and(root.or(dashboards).or(series)).or(data);

    warp::serve(routes).run(([127, 0, 0, 1], 3000));
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

