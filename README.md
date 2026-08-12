# Light Stripe

<p align="center">
  <img src="assets/light-stripe-concept.png" alt="Light Stripe — a bright stripe through the dark of local-dev chaos" width="820" />
</p>

<p align="center">
  <em>A bright stripe through the mess of local development.</em>
</p>

A small Rust tool for the everyday chaos of ports, processes, and Docker.

You know the feeling: Redis is up, Postgres is up, three `node` processes are running, something grabbed port `8080`, and `htop` is somehow both too much and not enough. Light Stripe is meant for that moment — a focused look at **ports**, **dev processes**, and **Docker**, plus a simple TUI when you want to poke around interactively.

---

## What it does

- Shows which processes (and Docker containers) are listening on which ports
- Lists “dev-looking” processes with CPU and memory
- Shows Docker containers with CPU / MEM, and lets you stop / restart / remove them
- Works as a CLI for scripting (`--format json`) or as a live TUI dashboard

Right now it works best on **Linux and WSL**. macOS / Windows are on the radar.

---

## Getting started

You need a recent Rust toolchain. Docker is optional, but without it you lose container names on ports and the Docker tab.

```bash
git clone <your-repo-url> light-stripe
cd light-stripe
cargo build --release
```

The binary ends up at `target/release/light-stripe`. For day-to-day hacking, this is fine too:

```bash
cargo run -- tui
```

---

## Everyday commands

```bash
# Open the dashboard
cargo run -- tui        # same as: cargo run -- ui

# Who is on 8080?
cargo run -- ports --port 8080

# Only the noisy local-dev stuff
cargo run -- ps --dev-only

# Quick RAM / CPU snapshot
cargo run -- stats

# Pipe into jq / scripts
cargo run -- ports --format json
cargo run -- ps -d --format json
```

---

## CLI

```text
light-stripe tui      Interactive TUI          (alias: ui)
light-stripe ports    Listening ports          (aliases: p, port)
light-stripe ps       Processes                (alias: proc)
light-stripe stats    System RAM / CPU         (aliases: st, sys)
```

### ports

```bash
light-stripe ports
light-stripe ports -p 6379
light-stripe ports --format json
```

Each row is a listening socket. The **OWNER** column is the useful bit: either a process name, or something like `redis-dev (docker)` when Docker published that host port.

### ps

```bash
light-stripe ps
light-stripe ps -d              # --dev-only
light-stripe ps --format json
```

Without `-d` you get a broader process list. With `-d`, Light Stripe tries to keep only things that look like local development (Node, Python, Cargo, local DBs, and similar).

### stats

```bash
light-stripe stats
light-stripe stats --format json
```

Host memory and CPU. On WSL, the total RAM is whatever the WSL2 VM is allowed to use — not always the full Windows machine.

### tui

```bash
light-stripe tui
```

Full-screen view. Data refreshes about every two seconds (press `r` to refresh now).

---

## Using the TUI

Three tabs:

1. **Ports** — look up who owns a port (read-only)
2. **Processes** — your local servers / tooling; press `x` to kill the selected process
3. **Docker** — containers with CPU / MEM; stop, restart, or remove

<p align="center">
  <img src="assets/light-stripe-tui-docker.png" alt="Light Stripe TUI — Docker tab showing containers with CPU and memory usage" width="820" />
</p>

<p align="center">
  <em>Docker tab — containers with CPU / MEM and quick actions.</em>
</p>

### Keys you’ll actually use

| Key | What it does |
|-----|----------------|
| `q` / `Esc` | Quit |
| `r` | Refresh |
| `1` `2` `3` | Jump to a tab |
| `Tab` | Cycle tabs |
| `↑` `↓` or `k` `j` | Move around |
| `/` | Search and jump to a match |
| `n` / `N` | Next / previous match |
| `x` | Kill process (Processes tab) |
| `s` | Stop container (Docker tab) |
| `S` | Restart container (Docker tab) |
| `d` | Remove container — then `y` to confirm, `n` / `Esc` to cancel |
| `Space` | Toggle mark on current row (Docker / Processes) |
| `a` | Mark all on current tab |
| `A` | Unmark all on current tab |
| `s` / `S` / `d` / `x` | Act on **marked** rows if any; otherwise the current row |

Search is intentionally simple: type a query, hit Enter, and the selection jumps to the first hit. It does not hide the rest of the list.

---

## Ports + Docker

If Docker is running, Light Stripe connects the dots for you:

```text
6379  →  redis-dev (docker)
5432  →  postgres-dev (docker)
8080  →  node
```

No Docker socket? Fine. Ports and processes still work; the Docker tab just tells you it couldn’t connect instead of taking the whole app down.

Seeing `::` in the ADDRESS column is normal — it usually means “listening on all interfaces,” which is common for published container ports.

---

## Project layout

Nothing fancy:

```text
collectors/   read from the OS and Docker
actions/      kill processes, stop/restart/remove containers
output/       CLI tables and JSON
tui/          the interactive UI
config.rs     user settings (TOML)
models.rs     shared types
```

Reading and changing things stay separate on purpose. Collectors don’t kill anything; the UI doesn’t talk to Docker directly for stop/remove.

### Config

Settings live in a TOML file under the OS config directory ([`directories`](https://docs.rs/directories) — `ProjectDirs::from("dev", "light-stripe", "light-stripe")`). If the file is missing, built-in defaults apply.

**Find the path on your machine:**

```bash
light-stripe config
# or from a clone:
cargo run -- config
```

The command prints the resolved path and effective values. When no file exists you will see `file not found, using defaults`.

**Typical config paths:**

| OS | Path |
|----|------|
| macOS | `~/Library/Application Support/dev.light-stripe.light-stripe/config.toml` |
| Linux / WSL | `~/.config/light-stripe/config.toml` |
| Windows | `%APPDATA%\light-stripe\light-stripe\config.toml` |

On Linux or Windows, run `light-stripe config` if your layout differs.

**Set up a custom config:**

1. Copy the example from the repo to the path shown by `light-stripe config`:

   ```bash
   CONFIG_DIR="$(light-stripe config 2>&1 | sed -n 's/^config path: //p' | xargs dirname)"
   mkdir -p "$CONFIG_DIR"
   cp config.example.toml "$CONFIG_DIR/config.toml"
   ```

   macOS one-liner (same path as above):

   ```bash
   mkdir -p ~/Library/Application\ Support/dev.light-stripe.light-stripe
   cp config.example.toml ~/Library/Application\ Support/dev.light-stripe.light-stripe/config.toml
   ```

2. Edit `docker_host`, `ignored_ports`, `extra_dev_markers`, `refresh_secs`, etc.

3. **Local override** without installing a user config:

   ```bash
   light-stripe --config ./config.toml tui
   cargo run -- --config ./config.toml config
   ```

**Docker (`docker_host`)**

Light Stripe uses the **local** Docker daemon on the machine where it runs. Usually you can leave `docker_host` unset — it auto-detects common socket paths.

Priority: config `docker_host` → env `DOCKER_HOST` → auto-detect.

If auto-detect fails, set in config:

```toml
docker_host = "unix:///Users/you/.docker/run/docker.sock"
```

(`/Users/you/.docker/run/docker.sock` without the `unix://` prefix also works.) Or export `DOCKER_HOST` when `docker_host` is unset in config.

### Libraries

Built with clap, ratatui, crossterm, sysinfo, netstat2, bollard, comfy-table, serde, toml, and directories.

---

## Things worth knowing

- Killing processes or talking to Docker needs the usual permissions. If something fails, the footer should say so.
- Docker CPU % is “share of the host,” same idea as `docker stats` — not “percent of the container’s CPU limit.”
- Linux / WSL first. Broader platform support comes later.

---

## Roadmap

Ideas, not promises — subject to change as the tool grows.

- **Richer Docker stats** — go beyond the current CPU / MEM columns. Volumes, disk usage, memory details (limits vs usage), and whatever else turns out useful without cluttering the main table. Details still TBD.
- Better macOS / Windows support
- Jump from a port row straight to its process or container (done)
- CLI helpers for kill / docker stop (same actions as the TUI) (done)

---

## License

MIT — see [LICENSE](LICENSE).
