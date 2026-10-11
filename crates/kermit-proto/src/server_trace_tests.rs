//! Replays of traces recorded with the emulated 38G and 39G as the client
//! of a [`Server`](crate::Server) (saturnus-tx's aplet server, `just
//! tx-aplet-traces`): our side must produce the recorded bytes exactly;
//! the calculator's side is fed back in the recorded chunks. The files we
//! served are read back out of the trace's own packets.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::cell::RefCell;
use std::collections::{BTreeMap, VecDeque};

use crate::codec::{BlockCheck, Deframer, parse_frame};
use crate::prefix::{self, Quoting};
use crate::server::test_support::replay_server;
use crate::trace::{self, Direction};
use crate::{ServerConfig, ServerEvent};

const G38: &str = include_str!("../traces/38g-serve.trace");
const G39: &str = include_str!("../traces/39g-serve.trace");

/// The files our side sent in `text`, by the name in their F packets, in
/// the order they went (a directory file goes out more than once).
fn served(text: &str) -> BTreeMap<Vec<u8>, VecDeque<Vec<u8>>> {
    let mut deframer = Deframer::new();
    for (dir, bytes) in trace::parse(text).unwrap() {
        if dir == Direction::Out {
            deframer.push(&bytes);
        }
    }
    let mut files = BTreeMap::new();
    let mut current: Option<(Vec<u8>, Vec<u8>)> = None;
    while let Some(frame) = deframer.next_frame() {
        let Some(p) = [BlockCheck::Type1, BlockCheck::Type3]
            .into_iter()
            .find_map(|c| parse_frame(&frame, c).ok())
        else {
            continue;
        };
        let q = Quoting::default();
        match p.kind {
            b'F' => current = Some((prefix::decode(&p.data, &q).unwrap(), Vec::new())),
            b'D' => {
                if let Some((_, data)) = current.as_mut() {
                    data.extend(prefix::decode(&p.data, &q).unwrap());
                }
            }
            b'Z' => {
                if let Some((name, data)) = current.take() {
                    files
                        .entry(name)
                        .or_insert_with(VecDeque::new)
                        .push_back(data);
                }
            }
            _ => {}
        }
    }
    files
}

fn names(events: &[ServerEvent], f: fn(&ServerEvent) -> Option<&Vec<u8>>) -> Vec<String> {
    events
        .iter()
        .filter_map(f)
        .map(|n| String::from_utf8_lossy(n).into_owned())
        .collect()
}

fn check(text: &str, family: &str) {
    let files = RefCell::new(served(text));
    let list_out = files
        .borrow()
        .get(&b"L1000000.000"[..])
        .and_then(|q| q.front().cloned());
    let (server, events) = replay_server(ServerConfig::default(), text, &|name| {
        files
            .borrow_mut()
            .get_mut(name)
            .and_then(VecDeque::pop_front)
    });
    assert!(server.is_idle());
    let requested = names(&events, |e| match e {
        ServerEvent::Request { name } => Some(name),
        _ => None,
    });
    let cur = format!("{family}DIR.CUR");
    let dir = format!("{family}DIR.000");
    assert_eq!(
        requested,
        vec![
            cur.clone(),
            dir.clone(),
            cur,
            dir.clone(),
            "L1000000.000".to_string()
        ]
    );
    let received = names(&events, |e| match e {
        ServerEvent::Received { name, .. } => Some(name),
        _ => None,
    });
    assert_eq!(received, vec!["L1000000.000".to_string(), dir]);
    // The list came in as it went out again.
    let list = events.iter().find_map(|e| match e {
        ServerEvent::Received { name, data } if name == b"L1000000.000" => Some(data),
        _ => None,
    });
    assert_eq!(list, list_out.as_ref());
    let header = format!("{family}BinE");
    assert!(list.unwrap().starts_with(header.as_bytes()));
    assert!(
        !events
            .iter()
            .any(|e| matches!(e, ServerEvent::Error(_) | ServerEvent::Remote(_)))
    );
}

#[test]
fn g38_list_to_the_folder_and_back() {
    check(G38, "HP38");
}

#[test]
fn g39_list_to_the_folder_and_back() {
    check(G39, "HP39");
}
