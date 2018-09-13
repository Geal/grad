use std::thread::{self, JoinHandle};
use std::sync::{Arc,Mutex};
use std::str;
use bytes::BytesMut;
use tokio;
use tokio::io;
use tokio::net::{UdpFramed, UdpSocket};
use tokio::prelude::*;
use tokio_io::codec::Decoder;
use nom::Offset;
use std::net::SocketAddr;

use super::metrics;

mod parser;

pub fn start(metrics: Arc<Mutex<metrics::Metrics>>, addr: SocketAddr) -> JoinHandle<()> {
    thread::spawn(move || {
        let socket = UdpSocket::bind(&addr).unwrap();
        let stream = UdpFramed::new(socket, StatsdCodec::new());

        let server = stream
            .for_each(move |msg| {
                //info!("msg: {:?}", msg);

                {
                  let mut m = metrics.lock().unwrap();
                  m.insert(msg.0);

                  //info!("metrics:\n{:?}", *m);
                }
                Ok(())
            })
            .map_err(|e| {
                error!("{:?}", e);
            });

        info!("starting statsd");
        tokio::run(server);
    })
}

#[derive(Clone,Debug,PartialEq)]
pub struct StatsdMessage {
  pub key: String,
  pub tags: Vec<Tag>,
  pub value: isize,
  pub metric_type: MetricType,
}

#[derive(Clone,Debug,PartialEq,Eq,Hash,Serialize)]
pub struct Tag {
  pub key:   String,
  pub value: String,
}

#[derive(Clone,Copy,Debug,PartialEq,Serialize)]
pub enum MetricType {
  Counter,
  Timing,
  Gauge,
}

pub struct StatsdCodec {
}

impl StatsdCodec {
  pub fn new() -> StatsdCodec {
    StatsdCodec {}
  }
}

impl Decoder for StatsdCodec {
    type Item = StatsdMessage;
    type Error = io::Error;

    fn decode(&mut self, buf: &mut BytesMut) -> Result<Option<Self::Item>, Self::Error> {
        trace!("got message: '{}'", str::from_utf8(buf).unwrap());
        //let len = buf.len();
        //buf.split_to(len);
        let (consumed, msg) = match parser::parse(buf) {
            Err(e) => {
                if e.is_incomplete() {
                    return Ok(None);
                } else {
                    return Err(io::Error::new(
                        io::ErrorKind::Other,
                        format!("parse error: {:?}", e),
                    ));
                }
            }
            Ok((i, message)) => (buf.offset(i), message),
        };

        trace!("decoded: {:?}", msg);
        buf.split_to(consumed);
        Ok(Some(msg))
    }
}


