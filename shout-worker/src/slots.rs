// shout.sh — curl-friendly ANSI banner service
// Copyright (C) 2026 Ryan Lewis
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

//! Per-client cap on open animation streams. One Durable Object per
//! client (`app::rate_limit_key`) holds that client's stream slots. The
//! object and the Worker call are in `glue`; the bookkeeping is here, as
//! plain Rust, so the host tests cover it.
//!
//! The object is named with `object_name`, a keyed hash of the client
//! key, so Cloudflare never sees the client's address as the name.
//!
//! A slot is a lease: it ends when the Worker releases it, or when it
//! expires. The expiry covers a crashed isolate or a stream that stops
//! being pulled without a disconnect, so a lost release cannot hold a
//! slot for longer than one stream's timeout plus `LEASE_MARGIN_MS`.

/// Most animation streams one client may hold open at once.
pub const MAX_STREAMS: usize = 3;

/// Added to a stream's timeout to get its lease. The stream's clock
/// starts on the first frame, a little after the slot is taken; the
/// margin covers that gap.
pub const LEASE_MARGIN_MS: u64 = 30_000;

/// Added to Retry-After. A slot frees a little after the stream's
/// timeout: the first frame comes after the slot is taken, and the
/// release runs in the background once the stream ends.
pub const RETRY_SLACK_SECS: u64 = 2;

/// One held slot.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Lease {
    pub id: u64,
    pub expires_ms: u64,
}

/// The leases one client holds. This is all the object stores: slot ids
/// and expiry times.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Slots {
    leases: Vec<Lease>,
}

impl Slots {
    pub fn leases(&self) -> &[Lease] {
        &self.leases
    }

    pub fn is_empty(&self) -> bool {
        self.leases.is_empty()
    }

    /// Drop every lease that expired at or before `now_ms`.
    pub fn prune(&mut self, now_ms: u64) {
        self.leases.retain(|l| l.expires_ms > now_ms);
    }

    /// Take a slot for `ttl_ms`. Over the cap, the error is the number of
    /// seconds to wait before retrying: until the oldest stream reaches its
    /// timeout (its lease minus the margin), rounded up, plus
    /// `RETRY_SLACK_SECS`. Taking an id that is already held renews it.
    pub fn acquire(&mut self, id: u64, now_ms: u64, ttl_ms: u64) -> Result<(), u64> {
        self.prune(now_ms);
        let expires_ms = now_ms.saturating_add(ttl_ms);
        if let Some(l) = self.leases.iter_mut().find(|l| l.id == id) {
            l.expires_ms = expires_ms;
            return Ok(());
        }
        if self.leases.len() >= MAX_STREAMS {
            let first = self.next_expiry().unwrap_or(now_ms);
            let ends_ms = first.saturating_sub(LEASE_MARGIN_MS);
            let wait = ends_ms.saturating_sub(now_ms).div_ceil(1000);
            return Err(wait + RETRY_SLACK_SECS);
        }
        self.leases.push(Lease { id, expires_ms });
        Ok(())
    }

    /// Give a slot back. Releasing an unknown or expired id does nothing.
    pub fn release(&mut self, id: u64, now_ms: u64) {
        self.prune(now_ms);
        self.leases.retain(|l| l.id != id);
    }

    /// When the next lease expires, for the object's cleanup alarm.
    pub fn next_expiry(&self) -> Option<u64> {
        self.leases.iter().map(|l| l.expires_ms).min()
    }

    /// Stored form: `id:expiry` pairs in hex and decimal, comma separated.
    pub fn encode(&self) -> String {
        let pairs: Vec<String> = self
            .leases
            .iter()
            .map(|l| format!("{:x}:{}", l.id, l.expires_ms))
            .collect();
        pairs.join(",")
    }

    /// Read `encode`'s form back. Pairs that do not parse are dropped.
    pub fn decode(s: &str) -> Self {
        let leases = s
            .split(',')
            .filter_map(|pair| {
                let (id, exp) = pair.split_once(':')?;
                Some(Lease {
                    id: u64::from_str_radix(id, 16).ok()?,
                    expires_ms: exp.parse().ok()?,
                })
            })
            .collect();
        Self { leases }
    }
}

/// A request from the Worker to a client's slot object.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Call {
    Acquire { id: u64, ttl_ms: u64 },
    Release { id: u64 },
}

impl Call {
    /// The URL path and query that carry this call to the object.
    pub fn to_path(self) -> String {
        match self {
            Self::Acquire { id, ttl_ms } => format!("/acquire?id={id:x}&ttl={ttl_ms}"),
            Self::Release { id } => format!("/release?id={id:x}"),
        }
    }

    /// Parse `to_path`'s form. `None` for anything else.
    pub fn parse(path: &str, query: Option<&str>) -> Option<Self> {
        let mut id = None;
        let mut ttl_ms = None;
        for pair in query?.split('&') {
            match pair.split_once('=')? {
                ("id", v) => id = Some(u64::from_str_radix(v, 16).ok()?),
                ("ttl", v) => ttl_ms = Some(v.parse().ok()?),
                _ => return None,
            }
        }
        match path {
            "/acquire" => Some(Self::Acquire {
                id: id?,
                ttl_ms: ttl_ms?,
            }),
            "/release" if ttl_ms.is_none() => Some(Self::Release { id: id? }),
            _ => None,
        }
    }
}

/// The name of the `StreamSlots` object for client key `key`:
/// HMAC-SHA256 of the key under `secret`, in lowercase hex. Without the
/// secret the name cannot be turned back into an address, even though an
/// IPv4 key has only 2^32 possible values.
pub fn object_name(key: &str, secret: &[u8]) -> String {
    use hmac::{Hmac, Mac};
    use std::fmt::Write;

    // HMAC takes a key of any length, so this cannot fail.
    let mut mac = Hmac::<sha2::Sha256>::new_from_slice(secret).expect("HMAC takes any key length");
    mac.update(key.as_bytes());
    mac.finalize()
        .into_bytes()
        .iter()
        .fold(String::with_capacity(64), |mut hex, b| {
            let _ = write!(hex, "{b:02x}");
            hex
        })
}

/// The lease to ask for, for a stream that times out after `timeout_ms`.
/// The Worker passes the timeout the request asked for, before
/// `stream::cap`, so for a capped stream the lease and a refusal's
/// retry-after can overstate how long the slot is busy.
pub fn lease_ms(timeout_ms: u64) -> u64 {
    timeout_ms.saturating_add(LEASE_MARGIN_MS)
}
