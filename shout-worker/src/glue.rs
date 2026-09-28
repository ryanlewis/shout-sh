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
use worker::js_sys::Promise;
use worker::wasm_bindgen_futures::JsFuture;
use worker::web_sys::AbortSignal;
use worker::{
    AnalyticsEngineDataPointBuilder, AnalyticsEngineDataset, Context, Date, Delay, Env, Headers,
    Request, Response, ResponseBody, Result, event,
};

use crate::app::{self, Body, Reply};
use crate::event::Event;
use crate::stream::{Animation, Step};

/// Analytics Engine binding. Missing in local dev unless configured; every
/// write is best effort and never fails the request.
const EVENTS_BINDING: &str = "SHOUT_EVENTS";
const ASSETS_BINDING: &str = "ASSETS";

#[event(fetch)]
async fn fetch(req: Request, env: Env, _ctx: Context) -> Result<Response> {
    let start = Date::now().as_millis();
    let url = req.url()?;
    let headers = req.headers();
    let accept = headers.get("accept")?;
    let user_agent = headers.get("user-agent")?;
    let method = req.method();
    let reply = app::handle(&app::Request {
        method: method.as_ref(),
        path: url.path(),
        query: url.query(),
        accept: accept.as_deref(),
        user_agent: user_agent.as_deref(),
    });
    let sink = env.analytics_engine(EVENTS_BINDING).ok();
    respond(reply, &env, sink, start, req.inner().signal()).await
}

/// Resolves when the client disconnects. `request.signal` fires on
/// disconnect (the `enable_request_signal` compatibility flag). Once the
/// client is gone the runtime stops pulling the body but never cancels it,
/// so without this the stream state would sit in memory until the isolate
/// is recycled.
fn disconnected(signal: AbortSignal) -> JsFuture {
    JsFuture::from(Promise::new(&mut |resolve, _| {
        if signal.aborted() {
            let _ = resolve.call0(&resolve);
        } else {
            signal.set_onabort(Some(&resolve));
        }
    }))
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
        out.set(k, v)?;
    }
    let resp = match body {
        Body::Empty => Response::empty()?,
        Body::Text(s) => Response::from_bytes(s.into_bytes())?,
        Body::Asset(path) => match fetch_asset(env, &path).await {
            Some(body) => Response::from_body(body)?,
            None => {
                // The asset is missing from web/dist: the request asked
                // for a hashed name that no longer exists, or the build
                // shipped without it. Same bare 404 the old server gave.
                event.status = 404;
                write_event(sink.as_ref(), &event, start, 0);
                return Ok(Response::empty()?.with_status(404));
            }
        },
        Body::Stream(anim) => {
            let state = StreamState {
                anim: *anim,
                frames: 0,
                sink,
                event,
                start,
                disconnect: disconnected(signal),
            };
            let resp = Response::from_stream(stream::unfold(state, next_chunk))?;
            return Ok(resp.with_status(status).with_headers(out));
        }
    };
    write_event(sink.as_ref(), &event, start, 0);
    Ok(resp.with_status(status).with_headers(out))
}

/// Fetch `path` from Workers Static Assets. `None` for anything but a 200.
async fn fetch_asset(env: &Env, path: &str) -> Option<ResponseBody> {
    let assets = env.assets(ASSETS_BINDING).ok()?;
    // The host is ignored by the assets binding; only the path matters.
    let resp = assets
        .fetch(format!("https://assets.invalid{path}"), None)
        .await
        .ok()?;
    (resp.status_code() == 200).then(|| resp.body().clone())
}

/// Per-stream state. Dropping it writes the analytics data point, so the
/// frame count is recorded whether the stream timed out or the client
/// went away. The runtime cancels the response body when the client
/// disconnects, which drops this state and the pending `Delay` with it.
struct StreamState {
    anim: Animation,
    frames: u64,
    sink: Option<AnalyticsEngineDataset>,
    event: Event,
    start: u64,
    disconnect: JsFuture,
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
                if let Either::Right(_) = select(delay, &mut st.disconnect).await {
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
