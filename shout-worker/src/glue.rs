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
use worker::js_sys::{Function, Promise};
use worker::wasm_bindgen::JsValue;
use worker::wasm_bindgen_futures::JsFuture;
use worker::web_sys::AbortSignal;
use worker::{
    AnalyticsEngineDataPointBuilder, AnalyticsEngineDataset, Context, Date, Delay, Env, Headers,
    Request, Response, ResponseBody, Result, console_error, event,
};

use crate::app::{self, Body, Limit, Reply};
use crate::event::Event;
use crate::stream::{Animation, Step};

/// Analytics Engine binding. Missing in local dev unless configured; every
/// write is best effort and never fails the request.
const EVENTS_BINDING: &str = "SHOUT_EVENTS";
const ASSETS_BINDING: &str = "ASSETS";
/// Rate-limit bindings, one per `app::Limit`. See `allowed`.
const RATE_LIMIT_BINDING: &str = "RATE_LIMIT";
const STREAM_LIMIT_BINDING: &str = "STREAM_LIMIT";

#[event(fetch)]
async fn fetch(req: Request, env: Env, _ctx: Context) -> Result<Response> {
    let start = Date::now().as_millis();
    let url = req.url()?;
    let headers = req.headers();
    let accept = headers.get("accept")?;
    let user_agent = headers.get("user-agent")?;
    let method = req.method();
    let areq = app::Request {
        method: method.as_ref(),
        path: url.path(),
        query: url.query(),
        accept: accept.as_deref(),
        user_agent: user_agent.as_deref(),
    };
    let client_ip = headers.get("cf-connecting-ip")?;
    let reply = match app::rate_limit(&areq) {
        Some((limit, route)) if !allowed(&env, limit, client_ip.as_deref()).await => {
            app::too_many_requests(route)
        }
        _ => app::handle(&areq),
    };
    let sink = env.analytics_engine(EVENTS_BINDING).ok();
    respond(reply, &env, sink, start, req.inner().signal()).await
}

/// Ask the rate-limit binding whether this client may go on. Fails open:
/// a missing binding, a missing client IP or a binding error all allow
/// the request. Binding problems are logged, so a broken limit shows up
/// in the Worker logs instead of passing silently.
async fn allowed(env: &Env, limit: Limit, client_ip: Option<&str>) -> bool {
    let binding = match limit {
        Limit::General => RATE_LIMIT_BINDING,
        Limit::Stream => STREAM_LIMIT_BINDING,
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

/// Per-stream state. Dropping it writes the analytics data point, so the
/// frame count is recorded whether the stream timed out or the client
/// went away. On disconnect `next_chunk` stops waiting and returns `None`,
/// which drops this state and clears the pending `Delay`'s timer.
struct StreamState {
    anim: Animation,
    frames: u64,
    sink: Option<AnalyticsEngineDataset>,
    event: Event,
    start: u64,
    disconnect: Disconnect,
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
