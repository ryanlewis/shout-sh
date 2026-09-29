// shout.sh — curl-friendly ANSI banner service
// Copyright (C) 2026 Ryan Lewis
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

//! Worker entry point. Turns a `worker::Request` into an `app::Request`,
//! and an `app::Reply` into a `worker::Response`. Logic that can run on
//! the host lives in `app`, `stream` and `event`.

use std::time::Duration;

use futures_util::future::{Either, select};
use futures_util::stream;
use worker::js_sys::{self, Function, Promise};
use worker::wasm_bindgen::{self, JsValue};
use worker::wasm_bindgen_futures::JsFuture;
use worker::wasm_bindgen_futures::future_to_promise;
use worker::web_sys::AbortSignal;
use worker::{
    AnalyticsEngineDataPointBuilder, AnalyticsEngineDataset, Context, Date, Delay, DurableObject,
    Env, Headers, Request, Response, ResponseBody, Result, ScheduledTime, State, Stub,
    console_error, durable_object, event,
};

use crate::app::{self, Body, Limit, Reply};
use crate::event::Event;
use crate::slots::{self, Call, Slots};
use crate::stream::{Animation, Step};

/// Analytics Engine binding. Missing in local dev unless configured; every
/// write is best effort and never fails the request.
const EVENTS_BINDING: &str = "SHOUT_EVENTS";
const ASSETS_BINDING: &str = "ASSETS";
/// Rate-limit bindings, one per `app::Limit`. See `allowed`.
const RATE_LIMIT_BINDING: &str = "RATE_LIMIT";
const STREAM_LIMIT_BINDING: &str = "STREAM_LIMIT";
/// Durable Object namespace for `StreamSlots`.
const STREAM_SLOTS_BINDING: &str = "STREAM_SLOTS";
/// Secret for `slots::object_name`. Set with `wrangler secret put`, or in
/// .dev.vars for local dev.
const SLOT_KEY_SECRET: &str = "SLOT_KEY_SECRET";
/// Longest wait for a slot before the stream goes ahead without one.
const SLOT_TIMEOUT: Duration = Duration::from_secs(1);

#[event(fetch)]
async fn fetch(req: Request, env: Env, ctx: Context) -> Result<Response> {
    let start = Date::now().as_millis();
    let url = req.url()?;
    let headers = req.headers();
    let accept = headers.get("accept")?;
    let user_agent = headers.get("user-agent")?;
    // The raw method, not `req.method()`: worker's `Method` turns any
    // method it does not know (PROPFIND, PURGE, ...) into GET, which
    // would get past the GET/HEAD check in `app`.
    let method = req.inner().method();
    let areq = app::Request {
        method: &method,
        path: url.path(),
        query: url.query(),
        accept: accept.as_deref(),
        user_agent: user_agent.as_deref(),
    };
    let client_ip = headers.get("cf-connecting-ip")?;
    let reply = match app::rate_limit(&areq) {
        Some((limit, route)) if !allowed(&env, limit, client_ip.as_deref()).await => {
            app::too_many_requests(limit, route)
        }
        _ => app::handle(&areq),
    };
    let mut slot = None;
    let reply = match (&reply.body, client_ip.as_deref()) {
        (Body::Stream(anim), Some(ip)) => {
            match acquire_slot(&env, ip, slots::lease_ms(anim.timeout_ms())).await {
                SlotOutcome::Held(held) => {
                    slot = Some(held);
                    reply
                }
                SlotOutcome::Refused(retry_after) => app::too_many_streams(retry_after),
                SlotOutcome::Open => reply,
            }
        }
        _ => reply,
    };
    let sink = env.analytics_engine(EVENTS_BINDING).ok();
    let end = StreamEnd { ctx, slot };
    respond(reply, &env, sink, start, req.inner().signal(), end).await
}

/// Ask the rate-limit binding whether this client may go on. Fails open:
/// a missing binding, a missing client IP or a binding error all allow
/// the request. Binding problems are logged, so a broken limit shows up
/// in the Worker logs instead of passing silently.
async fn allowed(env: &Env, limit: Limit, client_ip: Option<&str>) -> bool {
    let binding = match limit {
        Limit::General => RATE_LIMIT_BINDING,
        Limit::Stream => STREAM_LIMIT_BINDING,
        // Not a rate limit; see acquire_slot.
        Limit::StreamSlots => return true,
    };
    let Some(ip) = client_ip else {
        return true;
    };
    let limiter = match env.rate_limiter(binding) {
        Ok(limiter) => limiter,
        Err(e) => {
            console_error!("rate limit binding {binding} unavailable: {e}");
            return true;
        }
    };
    match limiter.limit(app::rate_limit_key(ip)).await {
        Ok(outcome) => outcome.success,
        // Not the error text: it could echo the key, which is the IP.
        Err(_) => {
            console_error!("rate limit binding {binding} failed");
            true
        }
    }
}

enum SlotOutcome {
    Held(HeldSlot),
    /// Over the cap. Seconds until a slot should be free.
    Refused(u64),
    /// No slot was taken, but the stream may go ahead. See `acquire_slot`.
    Open,
}

/// A slot this request holds in the client's `StreamSlots` object.
struct HeldSlot {
    stub: Stub,
    id: u64,
}

/// Take a stream slot for the client at `ip`. Fails open: a missing or
/// empty secret, a missing binding, a failed or slow call, or an
/// unexpected answer all let the stream go ahead. After a failed or slow
/// call the stream still releases its id when it ends, in case the object
/// took it. Only binding and secret names are logged, never the key.
async fn acquire_slot(env: &Env, ip: &str, ttl_ms: u64) -> SlotOutcome {
    // Never fall back to naming the object by the key itself.
    let secret = match env.secret(SLOT_KEY_SECRET).map(|s| s.to_string()) {
        Ok(secret) if !secret.is_empty() => secret,
        _ => {
            console_error!("{SLOT_KEY_SECRET} unset; stream slots not checked");
            return SlotOutcome::Open;
        }
    };
    let name = slots::object_name(&app::rate_limit_key(ip), secret.as_bytes());
    let stub = match env
        .durable_object(STREAM_SLOTS_BINDING)
        .and_then(|ns| ns.get_by_name(&name))
    {
        Ok(stub) => stub,
        Err(_) => {
            console_error!("{STREAM_SLOTS_BINDING} binding unavailable");
            return SlotOutcome::Open;
        }
    };
    let id = match getrandom::u64() {
        Ok(id) => id,
        Err(_) => {
            console_error!("{STREAM_SLOTS_BINDING}: no random slot id");
            return SlotOutcome::Open;
        }
    };
    let url = slot_url(Call::Acquire { id, ttl_ms });
    let call = stub.fetch_with_str(&url);
    // A failed or slow call may still have taken the slot: the object can
    // answer after the timeout, or the answer can be lost on the way back.
    // The stream goes ahead either way, so hold on to the id and release
    // it when the stream ends. Releasing an id the object never took does
    // nothing.
    let answer = match select(Box::pin(call), Delay::from(SLOT_TIMEOUT)).await {
        Either::Left((answer, _)) => Some(answer),
        Either::Right(_) => None,
    };
    let mut resp = match answer {
        Some(Ok(resp)) => resp,
        Some(Err(_)) => {
            console_error!("{STREAM_SLOTS_BINDING} acquire failed");
            return SlotOutcome::Held(HeldSlot { stub, id });
        }
        None => {
            console_error!("{STREAM_SLOTS_BINDING} acquire timed out");
            return SlotOutcome::Held(HeldSlot { stub, id });
        }
    };
    match resp.status_code() {
        204 => SlotOutcome::Held(HeldSlot { stub, id }),
        429 => match resp.text().await.ok().and_then(|t| t.trim().parse().ok()) {
            Some(secs) => SlotOutcome::Refused(secs),
            None => SlotOutcome::Open,
        },
        code => {
            console_error!("{STREAM_SLOTS_BINDING} acquire answered {code}");
            SlotOutcome::Open
        }
    }
}

/// The host is ignored; the stub routes by object id.
fn slot_url(call: Call) -> String {
    format!("https://stream-slots.invalid{}", call.to_path())
}

/// Gives back the stream's slot, if it holds one, when dropped: when the
/// stream ends, or when the reply fails before the stream starts.
struct StreamEnd {
    ctx: Context,
    slot: Option<HeldSlot>,
}

impl Drop for StreamEnd {
    /// `Drop` cannot wait, so the release runs under `waitUntil`. If that
    /// fails, the lease expires on its own.
    fn drop(&mut self) {
        let Some(HeldSlot { stub, id }) = self.slot.take() else {
            return;
        };
        let promise = future_to_promise(async move {
            if stub
                .fetch_with_str(&slot_url(Call::Release { id }))
                .await
                .is_err()
            {
                console_error!("{STREAM_SLOTS_BINDING} release failed");
            }
            Ok(JsValue::UNDEFINED)
        });
        // Not `Context::wait_until`: it panics if waitUntil throws.
        if self.ctx.as_ref().wait_until(&promise).is_err() {
            console_error!("{STREAM_SLOTS_BINDING} release not scheduled");
        }
    }
}

/// `future` resolves when the client disconnects. `request.signal` fires
/// on disconnect (the `enable_request_signal` compatibility flag). Once the
/// client is gone the runtime stops pulling the body but never cancels it,
/// so without this the stream state would sit in memory until the isolate
/// is recycled.
struct Disconnect {
    signal: AbortSignal,
    resolve: Function,
    future: JsFuture,
}

impl Disconnect {
    fn new(signal: AbortSignal) -> Self {
        let mut resolve = None;
        // The executor runs synchronously, so `resolve` is set on return.
        let promise = Promise::new(&mut |res, _| resolve = Some(res));
        let resolve = resolve.expect("Promise executor runs synchronously");
        if signal.aborted() {
            let _ = resolve.call0(&JsValue::UNDEFINED);
        } else {
            signal.set_onabort(Some(&resolve));
        }
        Self {
            signal,
            resolve,
            future: JsFuture::from(promise),
        }
    }
}

impl Drop for Disconnect {
    /// A stream that ends on its own leaves the promise pending, and a
    /// `JsFuture` frees its callbacks only once its promise settles.
    /// Settle it here so they do not outlive the request.
    fn drop(&mut self) {
        self.signal.set_onabort(None);
        let _ = self.resolve.call0(&JsValue::UNDEFINED);
    }
}

async fn respond(
    reply: Reply,
    env: &Env,
    sink: Option<AnalyticsEngineDataset>,
    start: u64,
    signal: AbortSignal,
    end: StreamEnd,
) -> Result<Response> {
    let Reply {
        status,
        headers,
        body,
        mut event,
    } = reply;
    let out = Headers::new();
    for (k, v) in &headers {
        out.set(k, v.as_ref())?;
    }
    let resp = match body {
        Body::Empty => Response::empty()?,
        Body::Text(s) => Response::from_bytes(s.into_bytes())?,
        Body::Asset(path) => match fetch_asset(env, &path).await {
            Ok(body) => Response::from_body(body)?,
            Err(code) => {
                // 404: the request asked for a hashed name that no longer
                // exists, or the build shipped without it. Same bare 404
                // the old server gave. 502: the binding itself failed.
                event.status = code;
                write_event(sink.as_ref(), &event, start, 0);
                return Ok(Response::empty()?.with_status(code));
            }
        },
        Body::Stream(anim) => {
            let state = StreamState {
                anim: *anim,
                frames: 0,
                sink,
                event,
                start,
                disconnect: Disconnect::new(signal),
                _end: end,
            };
            let resp = Response::from_stream(stream::unfold(state, next_chunk))?;
            return Ok(resp.with_status(status).with_headers(out));
        }
    };
    write_event(sink.as_ref(), &event, start, 0);
    Ok(resp.with_status(status).with_headers(out))
}

/// Fetch `path` from Workers Static Assets. The error is the status to
/// reply with: 404 when the file is not there, 502 when the binding fails.
async fn fetch_asset(env: &Env, path: &str) -> std::result::Result<ResponseBody, u16> {
    let assets = env.assets(ASSETS_BINDING).map_err(|_| 502u16)?;
    // The host is ignored by the assets binding; only the path matters.
    let resp = assets
        .fetch(format!("https://assets.invalid{path}"), None)
        .await
        .map_err(|_| 502u16)?;
    match resp.status_code() {
        200 => Ok(resp.body().clone()),
        404 => Err(404),
        _ => Err(502),
    }
}

/// Per-stream state. Dropping it writes the analytics data point and
/// releases the stream slot, so both happen whether the stream timed out
/// or the client went away. On disconnect `next_chunk` stops waiting and returns `None`,
/// which drops this state and clears the pending `Delay`'s timer.
struct StreamState {
    anim: Animation,
    frames: u64,
    sink: Option<AnalyticsEngineDataset>,
    event: Event,
    start: u64,
    disconnect: Disconnect,
    /// Held for its `Drop`, which releases the stream slot.
    _end: StreamEnd,
}

impl Drop for StreamState {
    fn drop(&mut self) {
        write_event(self.sink.as_ref(), &self.event, self.start, self.frames);
    }
}

async fn next_chunk(mut st: StreamState) -> Option<(Result<Vec<u8>>, StreamState)> {
    loop {
        match st.anim.step(Date::now().as_millis()) {
            Step::Frame(s) => {
                st.frames += 1;
                return Some((Ok(s.into_bytes()), st));
            }
            Step::Wait(ms) => {
                let delay = Delay::from(Duration::from_millis(ms));
                if let Either::Right(_) = select(delay, &mut st.disconnect.future).await {
                    // Returning None drops `st`, which writes the event.
                    return None;
                }
            }
            Step::End(s) => return Some((Ok(s.into_bytes()), st)),
            Step::Done => return None,
        }
    }
}

fn write_event(sink: Option<&AnalyticsEngineDataset>, event: &Event, start: u64, frames: u64) {
    let Some(sink) = sink else { return };
    let duration = Date::now().as_millis().saturating_sub(start);
    let point = AnalyticsEngineDataPointBuilder::new()
        .indexes([event.route])
        .blobs(event.blobs())
        .doubles(event.doubles(duration, frames))
        .build();
    // Best effort: a failed write must never fail the request.
    let _ = sink.write_data_point(&point);
}

/// One object per client key (`app::rate_limit_key`), named with
/// `get_by_name` and `slots::object_name`, a keyed hash of the key. It stores the client's leases and nothing else: slot ids
/// and expiry times, under one key, deleted once the last lease ends. An
/// alarm at the next expiry clears leases that were never released.
#[durable_object]
pub struct StreamSlots {
    state: State,
}

/// Storage key for the encoded `Slots`.
const SLOTS_KEY: &str = "slots";

impl StreamSlots {
    async fn load(&self, now_ms: u64) -> Result<Slots> {
        let stored: Option<String> = self.state.storage().get(SLOTS_KEY).await?;
        let mut slots = Slots::decode(stored.as_deref().unwrap_or(""));
        slots.prune(now_ms);
        Ok(slots)
    }

    async fn save(&self, slots: &Slots) -> Result<()> {
        let storage = self.state.storage();
        match slots.next_expiry() {
            None => {
                storage.delete(SLOTS_KEY).await?;
                storage.delete_alarm().await
            }
            Some(at) => {
                storage.put(SLOTS_KEY, slots.encode()).await?;
                // A time, not an offset: `set_alarm` reads a bare i64 as
                // milliseconds from now.
                let at = js_sys::Date::new(&JsValue::from_f64(at as f64));
                storage.set_alarm(ScheduledTime::new(at)).await
            }
        }
    }
}

impl DurableObject for StreamSlots {
    fn new(state: State, _env: Env) -> Self {
        Self { state }
    }

    /// 204 when a slot was taken or released. 429 over the cap, with the
    /// seconds to wait as the body.
    async fn fetch(&self, req: Request) -> Result<Response> {
        let url = req.url()?;
        let Some(call) = Call::parse(url.path(), url.query()) else {
            return Ok(Response::empty()?.with_status(400));
        };
        let now = Date::now().as_millis();
        let mut slots = self.load(now).await?;
        let before = slots.clone();
        let refused = match call {
            Call::Acquire { id, ttl_ms } => slots.acquire(id, now, ttl_ms).err(),
            Call::Release { id } => {
                slots.release(id, now);
                None
            }
        };
        // A refusal or a release of an unknown id changes nothing, so skip
        // the write. Leases `load` pruned are stale in storage, but every
        // load prunes them and their alarm is already due.
        if slots != before {
            self.save(&slots).await?;
        }
        match refused {
            Some(secs) => Ok(Response::ok(secs.to_string())?.with_status(429)),
            None => Ok(Response::empty()?.with_status(204)),
        }
    }

    async fn alarm(&self) -> Result<Response> {
        let slots = self.load(Date::now().as_millis()).await?;
        self.save(&slots).await?;
        Response::empty()
    }
}
