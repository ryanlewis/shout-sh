# shout.sh

Transform your text into bold ASCII art banners with rainbow animations.

## Quick Start

```bash
# Static banner
curl shout.sh/HELLO+WORLD

# Animated party mode (5 second timeout)
curl shout.sh/p/DEPLOY+SUCCESS?t=5

# Custom font and color
curl "shout.sh/AWESOME?font=doom&color=rainbow"
```

## Features

- 🎨 Multiple ASCII art fonts (doom, standard, banner, etc.)
- 🌈 Animated rainbow effects with party mode
- 🎯 Simple curl-friendly API
- ⚡ Fast, lightweight Go implementation
- 🔧 Zero configuration needed

## API

### Endpoints

- `GET /{text}` - Generate static ASCII art
- `GET /p/{text}` or `/party/{text}` - Animated streaming mode
- `GET /fonts` - List available fonts
- `GET /help` - Usage information

### Query Parameters

| Parameter | Alias | Default | Description |
|-----------|-------|---------|-------------|
| `font` | `f` | `doom` | Font style |
| `color` | `c` | none | Color scheme (rainbow, fire, matrix, ocean, neon) |
| `timeout` | `t` | 0 | Animation timeout in seconds (0=infinite) |
| `speed` | `s` | 5 | Animation speed (1-10) |
| `align` | `a` | `left` | Text alignment (left, center, right) |
| `border` | `b` | none | Border style (single, double, rounded) |

## Development

### Prerequisites

- Go 1.24.6+
- Just (optional, for task automation)

### Setup

```bash
# Clone repository
git clone https://github.com/ryanlewis/shout-sh.git
cd shout-sh

# Install dependencies
go mod download

# Run tests
go test -v -race -cover ./...

# Build and run
go build -o shout .
./shout
```

### Configuration

Environment variables (optional):
- `SHOUT_PUBLIC_PORT` - Public API port (default: 8080)
- `SHOUT_ADMIN_PORT` - Admin endpoints port (default: 9090)
- `SHOUT_MAX_TEXT_LENGTH` - Maximum input text length (default: 100)
- `SHOUT_RATE_LIMIT` - Requests per minute (default: 100)

## Docker

```bash
# Build image
docker build -t shout-sh .

# Run container
docker run -p 8080:8080 shout-sh
```