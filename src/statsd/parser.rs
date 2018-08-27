use super::{StatsdMessage, Tag, MetricType};
use std::str::from_utf8;
use nom::{digit,is_alphanumeric};

named!(pub parse<StatsdMessage>,
  do_parse!(
    key:  map_res!(take_while!(is_key), from_utf8) >>
    tags: many0!(preceded!(char!(','), tag)) >>
    char!(':') >>
    value:value >>
    char!('|') >>
    metric_type: metric_type >>
    opt!(complete!(char!('\n'))) >>
    (StatsdMessage {
      key: key.to_string(),
      tags,
      value,
      metric_type,
    })
  )
);

fn is_key(c: u8) -> bool {
  is_alphanumeric(c) || c == b'.' || c == b'-' || c == b'_'
}

named!(tag<Tag>,
  dbg_dmp!(do_parse!(
    key: map_res!(take_while!(is_tag_key), from_utf8) >>
    char!('=') >>
    value: map_res!(take_while!(is_tag_value), from_utf8) >>
    (Tag {
      key: key.to_string(),
      value: value.to_string(),
    })
  ))
);

fn is_tag_key(c: u8) -> bool {
  is_alphanumeric(c) || c == b'_'
}

fn is_tag_value(c: u8) -> bool {
  is_alphanumeric(c) || c == b'.' || c == b'-' || c == b'_'
}

named!(value<isize>,
  dbg_dmp!(do_parse!(
    sign: opt!(one_of!("-+")) >>
    nb: flat_map!(digit, parse_to!(isize)) >>
    ({
      match sign {
        Some('-') => nb * -1,
        _         => nb,
      }
    })
  ))
);

named!(metric_type<MetricType>,
  alt!(
    tag!("g") => { |_| MetricType::Gauge } |
    tag!("c") => { |_| MetricType::Counter } |
    tag!("ms") => { |_| MetricType::Timing }
  )
);
