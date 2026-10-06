# Light Stripe

<p align="center">
  <img src="https://raw.githubusercontent.com/bazuly/light-stripe/main/assets/light-stripe-concept.png" alt="Light Stripe" width="820" />
</p>

<p align="center">
  <em>Ports, processes, and Docker — without the noise.</em>
</p>

---

# Overview

Light Stripe is a small local-dev monitor written in Rust. Use the **CLI** in scripts, or open the **TUI** when you want to poke around ports, processes, containers, and volumes.

**Platforms:** Linux, WSL, and macOS.  
**Docker is optional.** Without a daemon, ports and processes still work; Docker / Volumes just stay empty or show an error instead of crashing the app.

### What it does

- Listening ports with process or Docker owner
- Dev-looking processes (editors, runtimes, local DBs…) — mark and kill from the TUI
- Optional **All Processes** tab for everything else
- Docker containers: CPU / MEM, stop / restart / remove
- Docker volumes: size, in-use, linked containers; delete unused ones
- Table or JSON output for scripting
- Background refresh in the TUI so the UI stays responsive

<p align="center">
  <img src="https://raw.githubusercontent.com/bazuly/light-stripe/main/assets/light-stripe-tui-docker.png" alt="Light Stripe TUI — Docker tab" width="820" />
</p>

---

# Documentation

## Install

Needs a recent Rust toolchain ([rustup](https://rustup.rs/)).

### From crates.io

```bash
cargo install light-stripe
light-stripe tui
```

`cargo install` puts the binary in **`~/.cargo/bin`**. If the shell says `command not found`, that directory is not on your `PATH`.

**Fix (zsh — default on macOS):**

```bash
# add once to ~/.zshrc
export PATH="$HOME/.cargo/bin:$PATH"
```

Or, if you use rustup, make sure this line exists and is not commented out:

```bash
. "$HOME/.cargo/env"
```

Then reload the shell:

```bash
source ~/.zshrc
# or open a new terminal
which light-stripe
light-stripe --help
```

Until `PATH` is fixed you can still run:

```bash
~/.cargo/bin/light-stripe tui
```

### From source

```bash
git clone https://github.com/bazuly/light-stripe.git
cd light-stripe
cargo build --release
./target/release/light-stripe tui
```

Or without installing:

```bash
cargo run --release -- tui
```

---

## Quick start

```bash
light-stripe tui          # interactive dashboard
light-stripe ports        # listening ports
light-stripe ps -d        # dev-looking processes only
light-stripe stats        # host RAM / CPU
light-stripe config       # config path + effective settings
```

Global option:

```bash
light-stripe --config ./config.toml tui
```

---

## CLI

| Command | Aliases | Description |
|---------|---------|-------------|
| `tui` | `ui` | Interactive dashboard |
| `ports` | `p`, `port` | Listening ports |
| `ps` | `proc` | Processes |
| `stats` | `st`, `sys` | Host memory / CPU |
| `config` | | Print config path and values |

### `ports`

```bash
light-stripe ports
light-stripe ports -p 6379
light-stripe ports --format json
```

OWNER is a process name, or something like `redis-dev (docker)` when Docker published the port.  
`::` in ADDRESS usually means “all interfaces” (common for published container ports).

### `ps`

```bash
light-stripe ps
light-stripe ps -d              # --dev-only
light-stripe ps --format json
```

`-d` keeps processes that look like local development (Node, Python, Cargo, editors, local DBs, plus `extra_dev_markers` from config).

### `stats`

```bash
light-stripe stats
light-stripe stats --format json
```

On WSL, totals are for the WSL2 VM, not necessarily the whole Windows machine.

---

## TUI

```bash
light-stripe tui
```

### Tabs

Tabs are numbered dynamically (`1`…`N`) based on what is enabled in config.

1. **Ports** — who owns the port; `Enter` / `g` jumps to process or container  
2. **Dev Processes** — “dev” markers; mark rows, `x` to kill (confirm `y`)  
3. **All Processes** — everything else (optional; can be hidden in config)  
4. **Docker** — mark rows; stop / restart / remove  
5. **Volumes** — size / in-use; `Enter` / `g` jumps to a linked container; `d` deletes unused volumes (confirm)

Data refreshes on a timer (`refresh_secs`, default **3s**). Press `r` to refresh now. Collection runs off the UI thread.

### Hotkeys

| Key | Action |
|-----|--------|
| `q` / `Esc` | Quit (`Esc` also cancels a confirm prompt) |
| `r` | Refresh |
| `1`–`N` | Jump to tab |
| `Tab` | Cycle tabs |
| `↑` `↓` / `k` `j` | Move |
| `PgUp` / `PgDn` | Page |
| `Home` / `End` | First / last row |
| `/` | Search |
| `Enter` | Apply search (jump to first match) / jump from Ports or Volumes |
| `n` / `N` | Next / previous search match |
| `g` | Jump (same as Enter on Ports / Volumes) |
| `Space` | Toggle mark (`●` / `○`) |
| `a` / `A` | Mark all / unmark all (current tab) |
| `x` | Kill process(es) — confirm `y` |
| `s` / `S` | Stop / restart container(s) |
| `d` | Remove container(s) or unused volume(s) — confirm `y` |

If anything is marked, actions hit the marks; otherwise the current row. Marks use container id / pid / volume name, so refresh does not scramble them. In-use volumes are not deleted.

Search jumps to matching rows; it does not filter the list away. After a search, the footer shows `n/N: next/prev` when a query is active.

---

## Config

```bash
light-stripe config
```

Missing file → built-in defaults (that is fine).

| OS | Typical path |
|----|----------------|
| macOS | `~/Library/Application Support/dev.light-stripe.light-stripe/config.toml` |
| Linux / WSL | `~/.config/light-stripe/config.toml` |
| Windows | `%APPDATA%\light-stripe\light-stripe\config.toml` |

Copy the example:

```bash
# macOS
mkdir -p ~/Library/Application\ Support/dev.light-stripe.light-stripe
cp config.example.toml ~/Library/Application\ Support/dev.light-stripe.light-stripe/config.toml
```

### Useful keys

| Key | Default | Meaning |
|-----|---------|---------|
| `refresh_secs` | `3` | TUI auto-refresh interval |
| `ignored_ports` | `53, 323, 5353, 0` | Hidden on the Ports tab |
| `extra_dev_markers` | `[]` | Extra substrings treated as “dev” processes |
| `docker_host` | unset | Docker endpoint override |
| `show_all_processes` | `true` | Show / hide the All Processes tab |

```toml
refresh_secs = 3
ignored_ports = [53, 323, 5353, 0]

# Hide All Processes if you only care about Dev Processes
show_all_processes = false

# extra_dev_markers = ["webpack-dev-server", "my-legacy-app"]

# docker_host = "unix:///Users/you/.docker/run/docker.sock"
```

Bare socket path without `unix://` works too. Connection order: config `docker_host` → env `DOCKER_HOST` → auto-detect (Linux socket, Docker Desktop on Mac, etc.).

When `show_all_processes = false`, tab hotkeys renumber to the remaining visible tabs.

---

## Docker notes

Published ports show up as container owners, e.g. `6379 → redis-dev (docker)`.

- Stop / kill / remove need normal OS / Docker permissions; failures show in the footer.
- Docker CPU % is host share (like `docker stats`), not “percent of the container limit”.

---

## License

MIT — see [LICENSE](LICENSE).
