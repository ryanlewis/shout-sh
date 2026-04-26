---
description: Build and deploy shout-server to the production VM (shout-sh.exe.xyz)
argument-hint: [rollback]
---

> **Operator-only.** This command is wired to the production VM at `shout-sh.exe.xyz` and needs SSH + passwordless-sudo as the `exedev` user. If you've forked shout.sh, this recipe will fail at the `scp` step — write your own deploy for your infra and ignore this file (or delete it).

Cross-compile the Rust binary locally and ship it to the VM, then restart the service. The VM is artifacts-only (`/opt/shout-sh/bin/`) — source, node_modules, and `web/dist/` never live there. Front-end assets (HTML, bundled `main.js`, CSS, wasm) are embedded into the binary at build time via `include_bytes!` in `shout-server/src/server.rs`.

Cloudflare Worker `shout-sh-proxy` fronts `shout.sh` → `shout-sh.exe.xyz` (for the HTTP curl story). Deploying the VM binary is enough; the Worker doesn't need a redeploy unless `worker/` changes.

Mode: `$ARGUMENTS` (empty = deploy, `rollback` = restore previous binary).

## Rollback mode

If `$ARGUMENTS` is `rollback`:

1. Run:
   ```
   ssh shout-sh.exe.xyz 'cd /opt/shout-sh/bin && mv shout-server.prev shout-server && sudo systemctl restart shout-sh.service'
   ```
2. Verify: `curl -sf https://shout.sh/health`
3. Report the health response and stop.

## Deploy steps

1. **Pre-flight**
   - `git status --porcelain` — note uncommitted changes (warn, don't block; user may be iterating).
   - Confirm `cargo zigbuild`, `pnpm`, `wasm-pack`, `scp`, `ssh` are present. The rust target `x86_64-unknown-linux-gnu` must be installed (`rustup target list --installed`) — the `.2.39` suffix in the build command below is a zigbuild glibc pin, not a rustup target name.

2. **Build web assets** (produces `web/dist/` which the Rust binary embeds):
   ```
   just web-build
   ```
   - This runs `wasm-pack` → copies glue into `web/src/wasm-pkg/` → `pnpm build` → `web/dist/`. If you've already built and nothing in `web/` or `shout-wasm/` has changed, you can skip this; the `include_bytes!` paths will pick up the existing dist.

3. **Cross-compile release binary** (darwin/arm64 → linux/x86_64 glibc 2.39, matching the Ubuntu 24.04 VM):
   ```
   cargo zigbuild --release --target x86_64-unknown-linux-gnu.2.39 -p shout-server
   ```
   - Output: `target/x86_64-unknown-linux-gnu/release/shout-server`.
   - Verify: `file target/x86_64-unknown-linux-gnu/release/shout-server` reports `ELF 64-bit LSB ... x86-64 ... for GNU/Linux`. If it says Mach-O, the `--target` got dropped — abort.

4. **Ship binary**:
   ```
   scp target/x86_64-unknown-linux-gnu/release/shout-server shout-sh.exe.xyz:/tmp/shout-server.new
   ```
   - Web assets are embedded in the binary. There is no `dist/` to rsync to the VM.

5. **Atomic swap + restart**:
   ```
   ssh shout-sh.exe.xyz 'set -e; sudo mv /opt/shout-sh/bin/shout-server /opt/shout-sh/bin/shout-server.prev; sudo mv /tmp/shout-server.new /opt/shout-sh/bin/shout-server; sudo chown root:root /opt/shout-sh/bin/shout-server; sudo chmod 0755 /opt/shout-sh/bin/shout-server; sudo systemctl restart shout-sh.service'
   ```
   - `shout-server.prev` keeps the previous binary for `/deploy rollback`.

6. **Verify**:
   ```
   curl -sf https://shout.sh/health
   curl -sf -o /dev/null -w '%{http_code}\n' https://shout.sh/
   curl -sf -o /dev/null -w '%{http_code}\n' https://shout.sh/about
   curl -sf -o /dev/null -w '%{http_code}\n' https://shout.sh/privacy
   ```
   - `/health` returns `ok`. If any route misbehaves, check the service: `ssh shout-sh.exe.xyz 'sudo systemctl status shout-sh.service --no-pager | head -40'` and surface it. Do **not** auto-rollback — let the user decide.

7. **Report**: commit SHA that was shipped (`git rev-parse --short HEAD`), binary size, health response, and a one-line summary of what's new (if there are recent commits the user would care about).

## Notes

- The VM has no Rust, Node, or wasm-pack — never try to build there.
- `systemctl restart` and the `/opt/shout-sh/bin/` swap need sudo (file is owned by root); passwordless sudo is configured for the `exedev` user.
- Restarts are ~sub-second; there's no graceful drain. Streaming clients (rainbow/fire) will be cut mid-frame and need to reconnect — fine for an animation, worth knowing.
- Runtime config (`PORT`, `METRICS_ADDR` for the Tailscale-only metrics listener) lives in the `shout-sh.service` systemd unit on the VM, not in this repo. `systemctl restart` re-reads it; if you need to change an env var, edit the unit on the VM (`sudo systemctl edit shout-sh.service`) and restart — a binary deploy alone won't pick up new env values that aren't already set there.
- The Cloudflare Worker at `worker/` only needs `wrangler deploy` when `worker.js` or `wrangler.toml` changes; it is **not** part of this recipe.
