# shout.sh

a tiny http service that renders stylized ascii banners over `curl`. it
runs as a cloudflare worker.

```
$ curl shout.sh/HELLO
$ curl shout.sh/tiny/hello+world
$ curl shout.sh/red/alert
$ curl shout.sh/fire/boom
```

`rainbow` and `fire` animate by default — frames stream live to your
terminal. (if you're piping or redirecting, add `-N` to disable curl's
output buffering.)

## usage

everything lives in the url path. the first segment is an optional set of
`+`-joined directives; the rest is the text. spaces are `+`.

```
$ curl shout.sh/{directives}/{text}
```

directives are classified in order: **font**, **mode**, **color**. unknown
tokens are ignored. if no directive in the first segment matches, the whole
path is treated as text.

### fonts

13 fonts, courtesy of [cfonts]:

```
block (default), slick, tiny, grid, pallet, shade, chrome,
simple, simpleblock, 3d, simple3d, huge, console
```

```
$ curl shout.sh/tiny/hello+world
$ curl shout.sh/fonts         # list
$ curl shout.sh/fonts/block   # preview
```

### colors

naming a color implies solid mode:

```
$ curl shout.sh/red/hi
$ curl shout.sh/cyanbright/ok
```

available: `red`, `green`, `blue`, `yellow`, `cyan`, `magenta`, `white`,
`gray`, and a `*bright` variant of each.

### presets

curated multi-color palettes for fonts that support more than one color
layer (`block`, `chrome`, `3d`, etc. use two; `chrome` uses three). on
single-color fonts like `tiny` the first stop is used and the rest are
silently dropped, so the same preset name "just works" everywhere.

```
$ curl shout.sh/sunset/hi        # two-tone on block
$ curl shout.sh/ocean/3d/Hello   # preset + font
$ curl shout.sh/presets          # list presets
$ curl shout.sh/presets/sunset   # preview one
```

available: `sunset`, `ocean`, `mint`, `candy`, `matrix`, `mono`, `neon`,
`ember`. presets imply solid mode — combining one with `rainbow` or `fire`
lets the animated mode win.

### modes

```
$ curl shout.sh/solid/hi       # solid white (or pair with a color)
$ curl shout.sh/rainbow/hi     # animated hsl hue ring
$ curl shout.sh/fire/hi        # animated red/orange/yellow flicker
```

solid and bare colors never animate. rainbow and fire animate by default —
add `once` to force a static frame.

### animation

```
$ curl shout.sh/rainbow/hi
$ curl 'shout.sh/fire/boom?fps=20&timeout=30'
$ curl shout.sh/rainbow+once/hi       # single static frame
$ curl shout.sh/solid+animate/ok      # stream a still frame (pointless, works)
```

- `animate` — force animation on any mode.
- `once` — force a single static frame.
- `?fps=N` — frames per second. default 10, capped at 30.
- `?timeout=N` — seconds before the server closes the stream. default 60,
  capped at 300.

browsers (detected by `Accept: text/html` or `User-Agent: Mozilla/*`) are
sent a single static frame — a hung tab is not a good time.

### query params

```
$ curl 'shout.sh/hi?font=tiny&mode=fire&once'
$ curl 'shout.sh/HELLO?format=json'
```

supported: `font`, `mode`, `color`, `preset`, `format`, `animate`, `once`,
`fps`, `timeout`. query params override path directives.

`format=json` always returns a single static frame — json and animation
don't mix.

## endpoints

| path            | description          |
| --------------- | -------------------- |
| `/`             | plain-text help      |
| `/{text}`       | render text          |
| `/{dir}/{text}` | render with config   |
| `/fonts`        | list fonts           |
| `/fonts/{name}` | preview a font       |
| `/presets`      | list presets         |
| `/presets/{name}` | preview a preset   |
| `/health`       | health check         |

## playground

open [shout.sh](https://shout.sh) in a browser and type. the same rendering
pipeline that serves `curl` is compiled to wasm and runs locally — every
frame is rendered in your tab, no streaming, no round-trips. the page shows
the exact `curl` command for the current state so you can copy it and paste
into a terminal.

```
$ curl shout.sh/           # plain text help (curl)
$ curl -H 'Accept: text/html' shout.sh/   # the playground html
```

## development

```
$ just wasm-build     # cfonts → wasm32 via wasm-pack, for the playground
$ just web-build      # wasm-build + pnpm build of the ts client → web/dist/
$ just worker-install # pinned wrangler into shout-worker/node_modules
$ just worker-build   # the worker → shout-worker/build/ via worker-build
$ just dev            # esbuild watcher + wrangler dev on :8787
```

needs rust with the `wasm32-unknown-unknown` target, `wasm-pack`,
`worker-build` (`cargo install worker-build --version =0.8.6 --locked`),
node and pnpm.

the worker lives in `shout-worker/`. routing, parsing and stream framing are
plain rust in `src/app.rs` and `src/stream.rs`, so `cargo test --all` covers
them on the host. `src/glue.rs` is the wasm-only part: it reads the request,
fetches html, css, js and wasm from workers static assets (`web/dist/`), and
drives animation frames on a timer.

## justfile

```
$ just              # list targets
$ just test         # cargo test --all
$ just lint         # fmt check + clippy -D warnings (host and wasm32)
$ just smoke        # assets, streaming and HEAD through wrangler dev
$ just parity       # diff https://shout.sh against a running `just dev`
$ just ci           # web-build + lint + test + worker build
```

## deployment

pushes to `main` deploy through the `deploy` job in
`.github/workflows/ci.yml`, after ci passes. it runs `wrangler deploy` from
`shout-worker/` in the `production` environment, which holds
`CLOUDFLARE_API_TOKEN` and `CLOUDFLARE_ACCOUNT_ID`. without the token the job
builds and skips the upload.

`shout-worker/wrangler.toml` sets the routes (`shout.sh/*`, `www.shout.sh/*`),
a per-request cpu limit (needs the workers paid plan) and the
`SHOUT_EVENTS` analytics engine dataset. `src/event.rs` documents its
columns.

## license

`shout.sh` is licensed under the gnu general public license v3.0 or later.
see `LICENSE` for the full text.

built with [cfonts] (gpl-3.0-or-later). linking cfonts in-process makes the
combined work gpl-3 — fine for this project.

[cfonts]: https://github.com/dominikwilkowski/cfonts
