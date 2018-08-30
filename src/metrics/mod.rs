use std::collections::HashMap;
use super::statsd::{MetricType, Tag, StatsdMessage};
use time::{Duration, Timespec, get_time};

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
      let res = match q.aggregate.function {
        AggregateFunction::None => {
          serie.values.iter().fold(
            (Vec::new(), Vec::new()),
            |mut res, v| {
              if v.timestamp.sec > since {
                res.0.push(v.timestamp.sec);
                res.1.push(v.value);
              }
              res
            }
          )
        },
        AggregateFunction::Sum => {
          let start = Timespec::new(q.range.since as i64, 0);
          let mut agg = SumAggregator::from(
            serie.values.iter(),
            (get_time() - start) / (q.aggregate.points.unwrap_or(1000) as i32),
            start);

          agg.fold(
            (Vec::new(), Vec::new()),
            |mut res, v| {
              if v.timestamp.sec > since {
                res.0.push(v.timestamp.sec);
                res.1.push(v.value);
              }
              res
            }
          )
        },
      };

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
  #[serde(default)]
  pub aggregate: Aggregate,
  pub key: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Range {
  pub since: u32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Aggregate {
  pub points:   Option<u32>,
  pub function: AggregateFunction,
}

impl Default for Aggregate {
  fn default() -> Self {
    Aggregate {
      points: None,
      function: AggregateFunction::None,
    }
  }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AggregateFunction {
  Sum,
  None,
}

pub struct NoneAggregator<T: Iterator<Item=Value>> {
  iterator: T,
}

impl <T: Iterator<Item=Value>> NoneAggregator<T> {
  pub fn from(iterator: T) -> Self {
    NoneAggregator {
      iterator,
    }
  }
}

impl <T: Iterator<Item=Value>> Iterator for NoneAggregator<T> {
  type Item = Value;
  fn next(&mut self) -> Option<Self::Item> {
    self.iterator.next()
  }
}

pub struct SumAggregator<'a, T: 'a+Iterator<Item=&'a Value>> {
  iterator: T,
  span: Duration,
  start: Timespec,
  next: Timespec,
  value: Option<isize>,
}

impl <'a, T: Iterator<Item=&'a Value>> SumAggregator<'a, T> {
  pub fn from(iterator: T, span: Duration, start: Timespec) -> Self {
    let next = start + span;
    SumAggregator {
      iterator,
      span,
      start,
      next,
      value: None,
    }
  }
}

fn next_timestamp(mut start: Timespec, span: Duration, current: Timespec) -> Timespec {
  while start < current {
    start = start + span;
  }

  start
}

impl <'a, T: Iterator<Item=&'a Value>> Iterator for SumAggregator<'a, T> {
  type Item = Value;
  fn next(&mut self) -> Option<Self::Item> {
    loop {
      if self.value.is_none() {
        match self.iterator.next() {
          None => return None,
          Some(next_val) => {
            info!("sum aggregator, got {:?}, self.value = {:?}", next_val, self.value);
            self.value = Some(next_val.value);
            if next_val.timestamp > self.next {
              //FIXME: maybe we're further than next?
              self.start = next_timestamp(self.start, self.span, next_val.timestamp);
              self.next = self.start + self.span;
            }
          }
        }
      } else {
        match self.iterator.next() {
          None => return Some(Value {
            timestamp: self.start,
            tags: Vec::new(),
            value: self.value.take().unwrap(),
          }),
          Some(next_val) => {
            info!("sum aggregator, got {:?}, self.value = {:?}", next_val, self.value);
            if next_val.timestamp > self.next {
              let value = Value {
                timestamp: self.start,
                tags: Vec::new(),
                value: self.value.take().unwrap(),
              };

              //FIXME: maybe we're further than next?
              self.start = next_timestamp(self.start, self.span, next_val.timestamp);
              self.next = self.start + self.span;
              self.value = Some(next_val.value);
              return Some(value);
            } else {
              self.value.as_mut().map(|v| *v += next_val.value);
            }
          }
        }

      }
    }
  }
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

