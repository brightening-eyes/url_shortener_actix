# URL Shortener

A high-performance, production-grade URL shortening service built with **Rust** and the **actix-web** framework. It features an in-memory cache layer, SQLite persistence via Sea-ORM, structured logging, and auto-generated OpenAPI documentation.

## Features

- **Shorten URLs** — `POST /` accepts a JSON body with a `url` field, validates it, and returns an 8-character nanoid-based short URL.
- **Redirect** — `GET /{short_code}` issues a 301 redirect to the original long URL.
- **In-memory cache** — A thread-safe DashMap reduces database lookups on hot paths (cache-first, DB-fallback).
- **Swagger UI** — Interactive API documentation served at `/swagger-ui/` via utoipa + utoipauto.
- **Structured logging** — Tracing with environment-filtered subscriber (`RUST_LOG`).
- **Fast allocator** — Uses mimalloc (secure mode) as the global allocator.

## Quick Start

### Prerequisites

- [Rust](https://www.rust-lang.org/) (edition 2024; MSRV depends on dependencies)

### Setup

```sh
# Clone the repository
git clone <repo-url> && cd url_shortener

# Configure environment
cp .env.example .env    # or use the existing .env

# Apply database migrations
cargo run --manifest-path migration/Cargo.toml -- up
```

### Run the server

```sh
cargo run
```

The server starts on `http://0.0.0.0:8080` (configurable via `SERVER_ADDR`).

## API Reference

### `POST /` — Shorten a URL

**Request:**
```json
{
  "url": "https://example.com/very/long/url"
}
```

**Response (200):**
```json
{
  "short_url": "http://localhost:8080/abc12345"
}
```

If the URL is invalid, a `400 Bad Request` is returned.

### `GET /{short_code}` — Redirect to original URL

| Status | Meaning |
|--------|---------|
| `301 Found` | Redirects to the original long URL |
| `404 Not Found` | No entry for this short code |

### Interactive Docs

Open `http://localhost:8080/swagger-ui/` in your browser to test endpoints via Swagger UI.

## Architecture

```
┌──────────────┐     POST /      ┌───────────────────────────────────┐
│   Client     │ ──────────────> │  actix-web Server                 │
│  (Browser /  │ <────────────── │  (src/main.rs)                    │
│   curl)      │     301/JSON    │  - global allocator: mimalloc     │
└──────────────┘                 │  - tracing + env-filter logging   │
                                 └───────────┬───────────────────────┘
                                             │
                     ┌───────────────────────┼───────────────────────┐
                     │                       │                       │
               ┌─────▼─────┐          ┌──────▼──────┐        ┌──────▼──────┐
               │  Cache     │          │  DbService  │        │  Swagger UI  │
               │  Service   │          │  (Sea-ORM)  │        │  (utoipa)    │
               │  (DashMap) │          │  SQLite     │        │              │
               │  mem       │          │  (*.sqlite) │        │  /swagger-ui/│
               └────────────┘          └─────────────┘        └─────────────┘
```

| Layer | Crate | Role |
|-------|-------|------|
| **HTTP** | `actix-web` 4 | Request routing, JSON deserialization, response generation |
| **ORM** | `sea-orm` 1.1 | SQLite connection pooling, typed queries, migrations |
| **Cache** | `dashmap` 6 | Thread-safe in-memory store for hot-path lookups |
| **Validation** | `validator` | URL format validation on incoming requests |
| **Observability** | `tracing` + `tracing-subscriber` | Structured logging with `RUST_LOG` env filter |
| **Docs** | `utoipa` + `utoipa-swagger-ui` + `utoipauto` | OpenAPI 3.1 spec generation; auto-discovers routes via `#[utoipauto]` |
| **Allocator** | `mimalloc` (secure) | Fast, secure memory allocation |

## Project Structure

```
url_shortener/
├── .env                        # Environment variables (DATABASE_URL, SERVER_ADDR)
├── Cargo.toml                  # Workspace & main crate manifest
├── AGENTS.md                   # Developer reference (commands, structure)
├── db.sqlite                   # SQLite database (checked in)
├── src/
│   ├── main.rs                 # Server entrypoint, app wiring, swagger setup
│   ├── entities/               # Sea-ORM generated entities
│   │   ├── mod.rs
│   │   ├── prelude.rs
│   │   └── url.rs              # URL model: id, short_code, long_url, created_at
│   ├── routes/
│   │   ├── mod.rs
│   │   └── url.rs              # POST / and GET /{short_code} handlers
│   └── services/
│       ├── mod.rs
│       ├── cache.rs            # DashMap-based in-memory cache
│       └── db.rs               # Sea-ORM connection & query methods
└── migration/                  # sea-orm-migration sub-crate
    ├── Cargo.toml
    └── src/
        ├── lib.rs              # Migrator registration
        ├── main.rs             # CLI runner
        └── m20220101_000001_create_table.rs  # Creates the `url` table
```

## Configuration

All configuration is read from environment variables (loaded via `dotenv` from `.env`).

| Variable | Default | Description |
|----------|---------|-------------|
| `DATABASE_URL` | `sqlite:./db.sqlite?mode=rwc` | SQLite connection string |
| `SERVER_ADDR` | `0.0.0.0:8080` | Address and port to bind the HTTP server |
| `RUST_LOG` | *(none)* | Tracing filter (e.g., `info`, `debug`, `url_shortener=debug`) |

## Database

### Schema

```sql
CREATE TABLE url (
    id         INTEGER PRIMARY KEY AUTOINCREMENT,
    short_code TEXT    NOT NULL UNIQUE,
    long_url   TEXT    NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP
);
```

### Migrations

```sh
# Apply pending migrations
cargo run --manifest-path migration/Cargo.toml -- up

# Rollback last migration
cargo run --manifest-path migration/Cargo.toml -- down

# Drop and reapply all migrations
cargo run --manifest-path migration/Cargo.toml -- fresh

# Check migration status
cargo run --manifest-path migration/Cargo.toml -- status

# Generate a new migration
cargo run --manifest-path migration/Cargo.toml -- generate NAME
```

## Build

```sh
cargo build              # debug build
cargo build --release    # release build (LTO=fat, panic=abort, strip)
```

The release profile is tuned for production:
- **LTO** = `fat` (max link-time optimization)
- **Panic** = `abort` (no unwind tables)
- **Strip** = `symbols` (smaller binary)
- **Opt-level** = `3`

## Logging

Set `RUST_LOG` to control verbosity:

```sh
RUST_LOG=info cargo run
RUST_LOG=url_shortener=debug cargo run
RUST_LOG=trace cargo run
```

## Caching Strategy

Requests for `GET /{short_code}` follow a **cache-first** pattern:

1. Check DashMap in-memory cache → **cache HIT**: immediate 301 redirect
2. Cache miss → query SQLite → **DB HIT**: populate cache, return 301
3. DB miss → return `404 Not Found`

Writes (`POST /`) always insert into both the database and the cache simultaneously.

## Dependencies

| Crate | Version | Purpose |
|-------|---------|---------|
| `actix-web` | 4.11 | HTTP framework |
| `sea-orm` | 1.1 | ORM (SQLite, Tokio runtime) |
| `dashmap` | 6.1 | Thread-safe in-memory cache |
| `nanoid` | 0.5 | Short code generation (8 chars) |
| `utoipa` | 5.4 | OpenAPI spec generation |
| `utoipa-swagger-ui` | 9.0 | Swagger UI hosting |
| `utoipauto` | 0.2 | Automatic route discovery for utoipa |
| `validator` | 0.20 | URL input validation |
| `serde` / `serde_json` | 1.0 | JSON serialization |
| `tokio` | 1.47 | Async runtime |
| `tracing` / `tracing-subscriber` | 0.1 / 0.3 | Structured logging |
| `dotenv` | 0.15 | `.env` file loading |
| `mimalloc` | 0.1 | Global allocator (secure) |
