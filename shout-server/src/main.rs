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

use std::net::SocketAddr;

#[tokio::main]
async fn main() {
    let port: u16 = std::env::var("PORT")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(8080);
    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    let listener = match tokio::net::TcpListener::bind(addr).await {
        Ok(l) => l,
        Err(e) => {
            eprintln!("bind {addr} failed: {e}");
            std::process::exit(1);
        }
    };
    eprintln!("shout.sh listening on {addr}");

    // If METRICS_ADDR is set, serve /__metrics on a second listener bound
    // to that address only. Production ties this to the Tailscale IP so
    // public ingress cannot reach the metrics endpoint.
    if let Ok(metrics_addr) = std::env::var("METRICS_ADDR") {
        match metrics_addr.parse::<SocketAddr>() {
            Ok(ma) => match tokio::net::TcpListener::bind(ma).await {
                Ok(ml) => {
                    eprintln!("shout.sh metrics listening on {ma}");
                    tokio::spawn(async move {
                        if let Err(e) = axum::serve(ml, shout_server::metrics_app()).await {
                            eprintln!("metrics serve error: {e}");
                        }
                    });
                }
                Err(e) => {
                    eprintln!("bind metrics {ma} failed: {e}");
                    std::process::exit(1);
                }
            },
            Err(e) => {
                eprintln!("METRICS_ADDR={metrics_addr} invalid: {e}");
                std::process::exit(1);
            }
        }
    }

    if let Err(e) = axum::serve(listener, shout_server::app()).await {
        eprintln!("serve error: {e}");
        std::process::exit(1);
    }
}
