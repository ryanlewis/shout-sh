# shout.sh

A tiny HTTP server that renders FIGlet text, optionally colored or animated,
over `curl`. Think [parrot.live] for your own ASCII banners.

[parrot.live]: https://parrot.live

```
curl shout.sh/HELLO
curl shout.sh/slant/Hello+World
curl -N shout.sh/rainbow/Hi        # animated by default
curl shout.sh/rainbow+once/Hi      # single frame
```

## Usage

Everything happens in the URL path. The first segment is an optional set of
`+`-joined directives; the rest is the text. Spaces are `+`.

```
curl shout.sh/{directives}/{text}
```

Directives are classified in order: **font**, **mode**, **color**, **flag**,
**layout**, **width**. Unknown tokens are ignored. If no directive in the first
segment matches, the whole path is treated as text.

### Fonts

```
curl shout.sh/big/Hello+World
curl shout.sh/slant/Hello+World
curl shout.sh/fonts          # list all
curl shout.sh/fonts/slant    # preview one
```

### Color

Named color means solid mode:

```
curl shout.sh/red/Hi
curl shout.sh/cyan/Hi
```

Available: `red`, `green`, `blue`, `yellow`, `cyan`, `magenta`, `orange`,
`white`.

### Modes (shaders)

Dynamic shaders animate by default — use `curl -N` to disable curl's output
buffering so you see frames as they arrive:

```
curl -N shout.sh/rainbow/Hi
curl -N shout.sh/fire/Hi
curl -N shout.sh/matrix/Hi
```

Add `once` to render a single static frame instead:

```
curl shout.sh/rainbow+once/Hi
```

`solid` (and any bare color) never animates — every frame would be identical.

### Layout & width

```
curl shout.sh/slant+w120/Hi
curl shout.sh/full/Hi       # full-width spacing
curl shout.sh/kern/Hi       # kerning
curl shout.sh/smush/Hi      # smushing
```

### Query params

Query params override path directives:

```
curl "shout.sh/Hi?font=slant&mode=rainbow&width=100"
curl "shout.sh/Hi?format=json"
```

Supported: `font`, `layout`, `mode`, `color`, `width`, `animate`, `once`,
`fps`, `timeout`, `format`.

## Endpoints

| Path           | Description             |
| -------------- | ----------------------- |
| `/`            | Help page               |
| `/{text}`      | Render text             |
| `/fonts`       | List available fonts    |
| `/fonts/{name}`| Preview a font          |
| `/health`      | Health check            |

## Development

```
just              # list targets
just run          # go run . on :8080
just test         # go test -race
just lint         # golangci-lint
just ci           # lint + test + build
```

Built on [figgo](https://github.com/ryanlewis/figgo) (FIGfont rendering) and
[tint](https://github.com/ryanlewis/tint) (ANSI shaders).

## License

MIT
