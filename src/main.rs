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
use std::sync::{Arc,Mutex};
use std::fmt::Write;

fn main() {
    pretty_env_logger::init();
    let metrics = Arc::new(Mutex::new(metrics::Metrics::new()));
    let _handle = statsd::start(metrics.clone());

    let root = warp::index().map(|| {
        debug!("hello");
        include_str!("../assets/index.html")
    });

    let metrics_filter = warp::any().map(move || metrics.clone());
    let data = warp::path("data").and(warp::path::param())
      .and(warp::query::<metrics::Query>()).and(metrics_filter.clone()).and_then(send_data);

    let dashboards = warp::path("dashboards").and(warp::fs::dir("./conf"));

    let series = warp::path("series").and(metrics_filter).and_then(send_series);

    let routes = warp::get2().and(root.or(data).or(dashboards).or(series));

    warp::serve(routes).run(([127, 0, 0, 1], 3000));
}

fn send_data(key:String, q: metrics::Query, metrics: Arc<Mutex<metrics::Metrics>>) -> Result<impl warp::Reply, warp::Rejection> {
  match (*metrics.lock().unwrap()).query(&key, &q) {
    Some(data) => Ok(warp::reply::json(&data)),
    None       => Err(warp::reject::not_found()),
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

