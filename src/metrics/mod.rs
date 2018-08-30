use std::collections::HashMap;
use super::statsd::{MetricType, Tag, StatsdMessage};
use time::{Timespec, get_time};

#[derive(Clone,Debug,PartialEq)]
pub struct Metrics {
  series: HashMap<String, Serie>,
  counter: usize,
}

impl Metrics {
  pub fn new() -> Metrics {
    Metrics {
      series: HashMap::new(),
      counter: 0,
    }
  }

  pub fn insert(&mut self, msg: StatsdMessage) {
    let entry = self.series.entry(msg.key.clone()).or_insert(Serie::new(msg.metric_type));
    (*entry).insert(msg);
    self.counter += 1;
  }

  pub fn query(&self, q: &Query) -> Option<QueryResult> {
    let since = i64::from(q.range.since);
    self.series.get(&q.key).map(|serie| {
      let res = serie.values.iter().fold(
        (Vec::new(), Vec::new()),
        |mut res, v| {
          if v.timestamp.sec > since {
            res.0.push(v.timestamp.sec);
            res.1.push(v.value);
          }
          res
        }
      );

      QueryResult {
        timestamps: res.0,
        values: res.1,
      }
    })
  }

  pub fn state(&self) -> StateResult {
    let series = self.series.iter().map(|(key, serie)| {
      let h = serie.values.iter().fold(HashMap::new(), |mut h, value| {
        if !h.contains_key(&value.tags) {
          h.insert(value.tags.clone(), 0usize);
        }

        h.get_mut(&value.tags).map(|v| *v += 1);

        h
      });
      (key.clone(), (serie.metric_type, h))
    }).collect();

    StateResult {
      counter: self.counter,
      series
    }
  }
}

#[derive(Clone,Debug,PartialEq)]
pub struct Serie {
  pub metric_type: MetricType,
  pub values: Vec<Value>,
}

impl Serie {
  pub fn new(metric_type: MetricType) -> Serie {
    Serie {
      metric_type,
      values: Vec::new(),
    }
  }

  pub fn insert(&mut self, msg: StatsdMessage) {
    self.values.push(Value {
      timestamp: get_time(),
      tags: msg.tags,
      value: msg.value,
    })
  }
}

#[derive(Clone,Debug,PartialEq)]
pub struct Value {
  pub timestamp: Timespec,
  pub tags:  Vec<Tag>,
  pub value: isize,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Query {
  #[serde(default)]
  pub range: Range,
  pub key: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Range {
  pub since: u32,
}

#[derive(Clone,Debug,PartialEq,Serialize,Deserialize)]
pub struct QueryResult {
  pub timestamps: Vec<i64>,
  pub values: Vec<isize>,
}

#[derive(Clone,Debug,PartialEq,Serialize)]
pub struct StateResult {
  pub counter: usize,
  pub series: HashMap<String, (MetricType, HashMap<Vec<Tag>, usize>)>,
}

