# shout.sh

a tiny http server that renders stylized ascii banners over `curl`.

```
$ curl shout.sh/HELLO
$ curl shout.sh/tiny/hello+world
$ curl shout.sh/red/alert
$ curl shout.sh/fire/boom
```

phase 1 is static. phase 2 will add animation.

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

### modes

```
$ curl shout.sh/solid/hi       # solid white (or pair with a color)
$ curl shout.sh/rainbow/hi     # per-char bright palette
$ curl shout.sh/fire/hi        # red -> orange -> yellow gradient
```

dynamic modes are not animated in phase 1. they will be in phase 2.

### query params

```
$ curl 'shout.sh/hi?font=tiny&mode=fire'
$ curl 'shout.sh/HELLO?format=json'
```

supported: `font`, `mode`, `color`, `format`. query params override path
directives.

## endpoints

| path            | description          |
| --------------- | -------------------- |
| `/`             | plain-text help      |
| `/{text}`       | render text          |
| `/{dir}/{text}` | render with config   |
| `/fonts`        | list fonts           |
| `/fonts/{name}` | preview a font       |
| `/health`       | health check         |

## development

```
$ just         # list targets
$ just run     # cargo run on :8080
$ just test    # cargo test
$ just lint    # fmt check + clippy -D warnings
$ just ci      # lint + test + release build
```

`PORT` env var overrides the default `8080`.

## license

`shout.sh` is licensed under the gnu general public license v3.0 or later.
see `LICENSE` for the full text.

built with [cfonts] (gpl-3.0-or-later). linking cfonts in-process makes the
combined work gpl-3 — fine for this project.

[cfonts]: https://github.com/dominikwilkowski/cfonts
