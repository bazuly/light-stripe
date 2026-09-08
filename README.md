# Light Stripe

<p align="center">
  <img src="assets/light-stripe-concept.png" alt="Light Stripe" width="820" />
</p>

<p align="center">
  <em>Ports, processes, and Docker — without the noise.</em>
</p>

Local-dev monitor written in Rust. CLI for scripts, TUI when you want to poke around.

Works on **Linux, WSL, and macOS**. Docker is optional: without it, ports and processes still work; container-related bits just stay empty.

---

## Features

- Listening ports with process / Docker owner
- Dev-looking processes (CPU, memory); kill from the TUI
- Docker containers (CPU / MEM): stop, restart, remove
- Docker volumes (size, in use, linked containers); delete unused ones
- Table or JSON output for scripting
- Background refresh in the TUI so the UI stays responsive

---

## Build

Needs a recent Rust toolchain.

```bash
git clone <repo-url> light-stripe
cd light-stripe
cargo build --release
```

Binary: `target/release/light-stripe`.

```bash
cargo run -- tui
```

---

## Commands

```text
light-stripe tui      Interactive TUI     (alias: ui)
light-stripe ports    Listening ports     (aliases: p, port)
light-stripe ps       Processes           (alias: proc)
light-stripe stats    Host RAM / CPU      (aliases: st, sys)
light-stripe config   Config path + values
```

Examples:

```bash
cargo run -- tui
cargo run -- ports -p 8080
cargo run -- ps -d
cargo run -- stats
cargo run -- ports --format json
cargo run -- config
```

### ports

```bash
light-stripe ports
light-stripe ports -p 6379
light-stripe ports --format json
```

OWNER is either a process name or something like `redis-dev (docker)` when Docker published the port.

### ps

```bash
light-stripe ps
light-stripe ps -d              # --dev-only
light-stripe ps --format json
```

`-d` keeps things that look like local development (Node, Python, Cargo, local DBs, plus markers from config).

### stats

```bash
light-stripe stats
light-stripe stats --format json
```

Host memory and CPU. On WSL, totals are for the WSL2 VM, not necessarily the whole Windows machine.

---

## TUI

```bash
light-stripe tui
```

Four tabs:

1. **Ports** — who owns the port; `Enter` / `g` jumps to process or container  
2. **Processes** — mark rows, `x` to kill (confirm)  
3. **Docker** — mark rows; stop / restart / remove  
4. **Volumes** — size / in-use; `Enter` / `g` jumps to a linked container; `d` deletes unused volumes (confirm)

Data refreshes on a timer (`refresh_secs` in config, default 3s). Press `r` to refresh now. Collection runs off the UI thread.

<p align="center">
  <img src="assets/light-stripe-tui-docker.png" alt="Light Stripe TUI — Docker tab" width="820" />
</p>

### Keys

| Key | Action |
|-----|--------|
| `q` / `Esc` | Quit (`Esc` cancels a confirm prompt) |
| `r` | Refresh |
| `1`–`4` | Tabs |
| `Tab` | Cycle tabs |
| `↑` `↓` / `k` `j` | Move |
| `PgUp` / `PgDn` | Page |
| `Home` / `End` | First / last row |
| `/` | Search |
| `n` / `N` | Next / previous match |
| `Enter` / `g` | Jump (port → process/container; volume → container) |
| `Space` | Toggle mark (`●` / `○`) |
| `a` / `A` | Mark all / unmark all (current tab) |
| `x` | Kill process(es) — confirm `y` |
| `s` / `S` | Stop / restart container(s) |
| `d` | Remove container(s) or unused volume(s) — confirm `y` |

If anything is marked, actions hit the marks; otherwise the current row. Marks use container id / pid / volume name, so refresh doesn’t scramble them. In-use volumes are not deleted.

Search jumps to matches; it doesn’t filter the list away.

---

## Docker

When Docker is available, published ports show up as container owners, e.g.:

```text
6379  →  redis-dev (docker)
5432  →  postgres-dev (docker)
```

No socket / daemon? Ports and processes still work. Docker / Volumes tabs show an error instead of taking the app down.

`::` in ADDRESS usually means “all interfaces” — common for published container ports.

Connection order: config `docker_host` → env `DOCKER_HOST` → auto-detect (Linux socket, Docker Desktop on Mac, etc.).

---

## Config

```bash
light-stripe config
# or: cargo run -- config
```

Prints the config path and effective settings. Missing file → built-in defaults.

| OS | Typical path |
|----|----------------|
| macOS | `~/Library/Application Support/dev.light-stripe.light-stripe/config.toml` |
| Linux / WSL | `~/.config/light-stripe/config.toml` |
| Windows | `%APPDATA%\light-stripe\light-stripe\config.toml` |

Copy the example and edit:

```bash
# macOS
mkdir -p ~/Library/Application\ Support/dev.light-stripe.light-stripe
cp config.example.toml ~/Library/Application\ Support/dev.light-stripe.light-stripe/config.toml
```

Or pass a file for one run:

```bash
light-stripe --config ./config.toml tui
```

Useful keys: `refresh_secs`, `ignored_ports`, `extra_dev_markers`, `docker_host`.

```toml
docker_host = "unix:///Users/you/.docker/run/docker.sock"
```

Bare path without `unix://` works too.

---

## Layout

```text
collectors/   OS + Docker reads (incl. full snapshot)
actions/      kill / stop / restart / remove
output/       CLI tables + JSON
tui/          ratatui UI, background worker, state pieces
models/       shared types (incl. Snapshot)
config.rs     TOML settings
```

Collectors don’t change the system; actions do. The TUI talks to Docker through the worker, not from the render loop.

---

## Notes

- Stop / kill / remove need normal OS / Docker permissions; failures show in the footer.
- Docker CPU % is host share (like `docker stats`), not “percent of the container limit”.

---

## License

MIT — see [LICENSE](LICENSE).
