//! Technical literals retain hex digits, delimiters, compression and numeric prefix/port values.
use std::net::{Ipv4Addr, Ipv6Addr};

use super::{sequence_component, words, Text};
use crate::pyre::{re, LazyRe, R};

static LABELED: LazyRe = re!(
    r"\b(IPv4|IPv6|IP(?:[ \t]+address)?|MAC(?:[ \t]+address)?|UUID)([ \t]*:?[ \t]+|:)(\[[A-Z0-9:.%_-]+\](?::[0-9]{1,5})?|[A-Z0-9:.%/_-]+)",
    "i"
);
static BRACKET: LazyRe =
    re!(r"(?<!\w)\[[A-Fa-f0-9:]+(?:%[A-Za-z0-9_.-]+)?\](?::[0-9]{1,5})?(?!\w|:[0-9])");
static MAC: LazyRe = re!(
    r"(?<![\w:-])(?:[A-Fa-f0-9]{2}(?::[A-Fa-f0-9]{2}){5}|[A-Fa-f0-9]{2}(?:-[A-Fa-f0-9]{2}){5})(?![\w:-])"
);
static UUID: LazyRe =
    re!(r"(?<![\w-])[A-Fa-f0-9]{8}(?:-[A-Fa-f0-9]{4}){3}-[A-Fa-f0-9]{12}(?![\w-])");
static IPV6: LazyRe = re!(
    r"(?<![\w:])(?:[A-Fa-f0-9]{0,4}:){2,8}[A-Fa-f0-9]{0,4}(?:%[A-Za-z0-9_](?:[A-Za-z0-9_.-]*[A-Za-z0-9_])?)?(?:/[0-9]{1,3})?(?![\w:/])"
);
static IPV4: LazyRe = re!(
    r"(?<![\w.])(?:[0-9]{1,3}\.){3}[0-9]{1,3}(?::[0-9]{1,5}|/[0-9]{1,3})?(?!\w|\.[0-9]|[:/][0-9])"
);

fn literal(s: &str) -> String {
    let mut out = Vec::new();
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        let word = match c {
            ':' if chars.peek() == Some(&':') => {
                chars.next();
                "double colon".to_owned()
            }
            ':' => "colon".to_owned(),
            '-' => "dash".to_owned(),
            '.' => "dot".to_owned(),
            '%' => "percent".to_owned(),
            '_' => "underscore".to_owned(),
            c if c.is_ascii_digit() => words::digits(&c.to_string()),
            c => c.to_ascii_uppercase().to_string(),
        };
        out.push(word);
    }
    out.join(" ")
}

fn prefix(s: &str, maximum: u8) -> Option<(&str, Option<&str>)> {
    match s.split_once('/') {
        Some((value, width)) if width.parse::<u8>().is_ok_and(|n| n <= maximum) => {
            Some((value, Some(width)))
        }
        Some(_) => None,
        None => Some((s, None)),
    }
}

fn ipv6(s: &str) -> Option<String> {
    let (host, width) = prefix(s, 128)?;
    let address = host.split_once('%').map_or(host, |(ip, _)| ip);
    address.parse::<Ipv6Addr>().ok()?;
    let mut out = literal(host);
    if let Some(width) = width {
        out += &format!(" slash {}", sequence_component(width));
    }
    Some(out)
}

fn port(s: &str) -> Option<String> {
    s.parse::<u16>().ok()?;
    Some(format!(" port {}", sequence_component(s)))
}

fn ipv4(s: &str) -> Option<String> {
    let (value, width) = prefix(s, 32)?;
    let (host, service) = value
        .split_once(':')
        .map_or((value, None), |(a, b)| (a, Some(b)));
    host.parse::<Ipv4Addr>().ok()?;
    if service.is_some() && width.is_some() {
        return None;
    }
    let mut out = host
        .split('.')
        .map(sequence_component)
        .collect::<Vec<_>>()
        .join(" dot ");
    if let Some(service) = service {
        out += &port(service)?;
    }
    if let Some(width) = width {
        out += &format!(" slash {}", sequence_component(width));
    }
    Some(out)
}

fn bracket(s: &str) -> Option<String> {
    let (host, rest) = s.strip_prefix('[')?.split_once(']')?;
    let mut out = format!("open bracket {} close bracket", ipv6(host)?);
    if !rest.is_empty() {
        out += &port(rest.strip_prefix(':')?)?;
    }
    Some(out)
}

fn mac(s: &str) -> Option<String> {
    let delimiter = if s.contains(':') { ':' } else { '-' };
    let groups: Vec<_> = s.split(delimiter).collect();
    (groups.len() == 6
        && groups
            .iter()
            .all(|g| g.len() == 2 && g.bytes().all(|b| b.is_ascii_hexdigit())))
    .then(|| literal(s))
}

fn uuid(s: &str) -> Option<String> {
    let groups: Vec<_> = s.split('-').collect();
    let lengths: Vec<_> = groups.iter().map(|s| s.len()).collect();
    ((lengths == [8, 4, 4, 4, 12] || lengths == [32])
        && groups
            .iter()
            .all(|g| g.bytes().all(|b| b.is_ascii_hexdigit())))
    .then(|| literal(s))
}

pub(super) fn apply(t: &mut Text) -> R<()> {
    t.recognize(&LABELED, |m| {
        let value = m.g(3).trim_end_matches('.');
        let (label, reading) = match m.g(1).to_ascii_uppercase().as_str() {
            "IPV4" => ("I P version four", ipv4(value)),
            "IPV6" => ("I P version six", bracket(value).or_else(|| ipv6(value))),
            "IP" => ("I P", ipv4(value).or_else(|| ipv6(value))),
            "IP ADDRESS" => ("I P address", ipv4(value).or_else(|| ipv6(value))),
            "MAC" => ("M A C", mac(value)),
            "MAC ADDRESS" => ("M A C address", mac(value)),
            _ => ("U U I D", uuid(value)),
        };
        // Explicit malformed technical fields stay literal, never become clocks or generic quantities.
        if reading.is_none() && !value.chars().any(|c| c.is_ascii_digit() || c == ':') {
            return None;
        }
        Some(format!(
            "{label}{} {}{}",
            m.g(2).trim_end(),
            reading.unwrap_or_else(|| value.to_owned()),
            &m.g(3)[value.len()..]
        ))
    })?;
    t.recognize(&BRACKET, |m| bracket(m.all()))?;
    t.recognize(&MAC, |m| mac(m.all()))?;
    t.recognize(&UUID, |m| uuid(m.all()))?;
    t.recognize(&IPV6, |m| ipv6(m.all()))?;
    t.recognize(&IPV4, |m| ipv4(m.all()))
}
