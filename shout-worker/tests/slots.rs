// shout.sh — curl-friendly ANSI banner service
// Copyright (C) 2026 Ryan Lewis
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.

//! Stream slot bookkeeping and the 429 for too many open streams. The
//! Durable Object and its binding only run in the Worker; `just smoke`
//! covers them.

use shout_worker::app::rate_limit_key;
use shout_worker::app::{Body, Limit, Request, handle, too_many_streams};
use shout_worker::slots::{
    Call, LEASE_MARGIN_MS, Lease, MAX_STREAMS, RETRY_SLACK_SECS, Slots, lease_ms, object_name,
};

const MIN: u64 = 60_000;

fn full(now: u64, ttl: u64) -> Slots {
    let mut s = Slots::default();
    for id in 0..MAX_STREAMS as u64 {
        s.acquire(id, now, ttl).unwrap();
    }
    s
}

#[test]
fn acquire_up_to_the_cap_then_refuse() {
    let mut s = full(0, MIN);
    assert_eq!(s.leases().len(), MAX_STREAMS);
    assert!(s.acquire(99, 0, MIN).is_err());
    assert_eq!(s.leases().len(), MAX_STREAMS);
}

#[test]
fn release_frees_a_slot() {
    let mut s = full(0, MIN);
    s.release(1, 10);
    assert_eq!(s.acquire(99, 10, MIN), Ok(()));
    assert!(s.acquire(100, 10, MIN).is_err());
}

#[test]
fn release_of_unknown_id_changes_nothing() {
    let mut s = full(0, MIN);
    let before = s.clone();
    s.release(99, 0);
    assert_eq!(s, before);
}

#[test]
fn expired_leases_free_their_slots() {
    let mut s = full(0, MIN);
    assert!(s.acquire(99, MIN - 1, MIN).is_err());
    // A lease ends at its expiry time, not after it.
    assert_eq!(s.acquire(99, MIN, MIN), Ok(()));
    assert_eq!(
        s.leases(),
        &[Lease {
            id: 99,
            expires_ms: 2 * MIN
        }]
    );
}

#[test]
fn prune_keeps_live_leases() {
    let mut s = Slots::default();
    s.acquire(1, 0, 10).unwrap();
    s.acquire(2, 0, 20).unwrap();
    s.prune(10);
    assert_eq!(
        s.leases(),
        &[Lease {
            id: 2,
            expires_ms: 20
        }]
    );
    s.prune(20);
    assert!(s.is_empty());
}

#[test]
fn acquiring_a_held_id_renews_it() {
    let mut s = full(0, MIN);
    assert_eq!(s.acquire(0, 5, MIN), Ok(()));
    assert_eq!(s.leases().len(), MAX_STREAMS);
    assert_eq!(s.next_expiry(), Some(MIN));
    assert_eq!(s.leases()[0].expires_ms, MIN + 5);
}

#[test]
fn retry_after_is_when_the_oldest_stream_times_out() {
    // Three 60s streams with their leases, the first taken at t=0.
    let mut s = Slots::default();
    for (id, at) in [(1, 0), (2, 10_000), (3, 20_000)] {
        s.acquire(id, at, lease_ms(MIN)).unwrap();
    }
    // Stream 1 times out at t=60s; at t=25.5s that is 34.5s away.
    assert_eq!(
        s.acquire(9, 25_500, lease_ms(MIN)),
        Err(35 + RETRY_SLACK_SECS)
    );
}

#[test]
fn retry_after_is_at_least_the_slack() {
    // Past the oldest stream's timeout but inside its margin: the
    // release is late or lost, and the lease has not expired yet.
    let mut s = full(0, lease_ms(1000));
    assert_eq!(
        s.acquire(9, 1000 + LEASE_MARGIN_MS - 1, MIN),
        Err(RETRY_SLACK_SECS)
    );
}

#[test]
fn object_name_matches_rfc_4231() {
    // RFC 4231 test case 2: HMAC-SHA256, key "Jefe".
    assert_eq!(
        object_name("what do ya want for nothing?", b"Jefe"),
        "5bdcc146bf60754e6a042426089575c75a003f089d2739839dec58b964ec3843"
    );
}

#[test]
fn object_name_is_stable_and_hides_the_address() {
    let secret = b"0123456789abcdef0123456789abcdef";
    for ip in [
        "203.0.113.7",
        "2001:db8:1234:5678::1",
        "::ffff:198.51.100.9",
    ] {
        let key = rate_limit_key(ip);
        let name = object_name(&key, secret);
        assert_eq!(name, object_name(&key, secret), "{ip}");
        assert_eq!(name.len(), 64, "{ip}");
        assert!(name.bytes().all(|b| b.is_ascii_hexdigit()), "{name}");
        // Both have a '.' or ':', which hex never does. Shorter pieces
        // such as "203" can turn up in any hex string by chance.
        for part in [ip, key.as_str()] {
            assert!(!name.contains(part), "{name} contains {part}");
        }
    }
}

#[test]
fn object_name_depends_on_key_and_secret() {
    let a = object_name("203.0.113.7", b"one");
    assert_ne!(a, object_name("203.0.113.8", b"one"));
    assert_ne!(a, object_name("203.0.113.7", b"two"));
}

#[test]
fn lease_is_timeout_plus_margin() {
    assert_eq!(lease_ms(300_000), 300_000 + LEASE_MARGIN_MS);
    assert_eq!(lease_ms(u64::MAX), u64::MAX);
}

#[test]
fn next_expiry_is_the_earliest_lease() {
    let mut s = Slots::default();
    assert_eq!(s.next_expiry(), None);
    s.acquire(1, 0, 30).unwrap();
    s.acquire(2, 0, 10).unwrap();
    s.acquire(3, 0, 20).unwrap();
    assert_eq!(s.next_expiry(), Some(10));
}

#[test]
fn encode_round_trips() {
    let mut s = Slots::default();
    s.acquire(u64::MAX, 0, 5).unwrap();
    s.acquire(0xabc, 0, 1_800_000_000_000).unwrap();
    assert_eq!(s.encode(), "ffffffffffffffff:5,abc:1800000000000");
    assert_eq!(Slots::decode(&s.encode()), s);
}

#[test]
fn decode_skips_bad_pairs() {
    assert!(Slots::decode("").is_empty());
    let s = Slots::decode("1:5,zz:6,2:x,3,4:7");
    assert_eq!(
        s.leases(),
        &[
            Lease {
                id: 1,
                expires_ms: 5
            },
            Lease {
                id: 4,
                expires_ms: 7
            },
        ]
    );
}

#[test]
fn calls_round_trip_through_a_url() {
    for call in [
        Call::Acquire {
            id: 0xdead_beef,
            ttl_ms: 330_000,
        },
        Call::Release { id: u64::MAX },
    ] {
        let path = call.to_path();
        let (p, q) = path.split_once('?').unwrap();
        assert_eq!(Call::parse(p, Some(q)), Some(call), "{path}");
    }
}

#[test]
fn bad_calls_are_rejected() {
    for (path, query) in [
        ("/acquire", None),
        ("/acquire", Some("id=1")),
        ("/acquire", Some("ttl=1")),
        ("/acquire", Some("id=zz&ttl=1")),
        ("/acquire", Some("id=1&ttl=-1")),
        ("/acquire", Some("id=1&ttl=1&x=2")),
        ("/release", Some("id=1&ttl=1")),
        ("/release", Some("")),
        ("/other", Some("id=1")),
    ] {
        assert_eq!(Call::parse(path, query), None, "{path}?{query:?}");
    }
}

#[test]
fn too_many_streams_reply() {
    let r = too_many_streams(35);
    assert_eq!(r.status, 429);
    assert_eq!(r.header("content-type"), Some("text/plain; charset=utf-8"));
    assert_eq!(r.header("retry-after"), Some("35"));
    let Body::Text(body) = &r.body else {
        panic!("body: {:?}", r.body)
    };
    assert!(body.contains("too many open streams"), "{body}");
    assert_eq!(r.event.route, "/render");
    assert_eq!(r.event.status, 429);
    assert_eq!(r.event.error, "stream_slots");
    assert_eq!(Limit::StreamSlots.reason(), "stream_slots");
}

/// The glue only asks for a slot when the reply streams, so these are the
/// requests that count against the cap.
#[test]
fn only_animated_streams_take_a_slot() {
    let get = |uri: &'static str, method: &'static str, accept: Option<&'static str>| {
        let (path, query) = match uri.split_once('?') {
            Some((p, q)) => (p, Some(q)),
            None => (uri, None),
        };
        let r = handle(&Request {
            method,
            path,
            query,
            accept,
            user_agent: None,
        });
        matches!(r.body, Body::Stream(_))
    };
    assert!(get("/fire/boom", "GET", None));
    assert!(get("/animate/hi", "GET", None));
    assert!(!get("/fire/boom", "HEAD", None));
    assert!(!get("/fire/boom", "POST", None));
    assert!(!get("/fire/boom?format=json", "GET", None));
    assert!(!get("/fire+once/boom", "GET", None));
    assert!(!get("/fire/boom", "GET", Some("text/html")));
    assert!(!get("/hello", "GET", None));
}
