# Contui

[![CI](https://github.com/aycandv/contui/actions/workflows/ci.yml/badge.svg)](https://github.com/aycandv/contui/actions/workflows/ci.yml)
[![Release](https://github.com/aycandv/contui/actions/workflows/release.yml/badge.svg)](https://github.com/aycandv/contui/actions/workflows/release.yml)
[![GitHub release](https://img.shields.io/github/v/release/aycandv/contui?include_prereleases)](https://github.com/aycandv/contui/releases/latest)

[![Rust](https://img.shields.io/badge/rust-%23000000.svg?style=for-the-badge&logo=rust&logoColor=white)](https://www.rust-lang.org/)
[![Docker](https://img.shields.io/badge/docker-%230db7ed.svg?style=for-the-badge&logo=docker&logoColor=white)](https://www.docker.com/)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg?style=for-the-badge)](https://opensource.org/licenses/MIT)

A powerful, fast, and intuitive Terminal User Interface (TUI) for Docker container management. Contui provides real-time monitoring, log streaming, resource statistics, and full container lifecycle management - all from your terminal.

![Contui Demo](assets/demo.gif)

## Features

- 📊 **Real-time Monitoring**: Live container status, resource usage, and health checks
- 📜 **Log Streaming**: View and search container logs with real-time updates
- 🔍 **Log Search & Filter**: Search logs with regex, filter by level (INFO/WARN/ERROR), and time range
- 📈 **Container Stats**: CPU, memory, network I/O monitoring with live graphs
- 🔎 **Detailed Inspection**: View container and image details (ports, mounts, env vars, labels, layers)
- 🧩 **Exec Shell**: Open an interactive shell inside a container (toggle focus with Ctrl+E)
- 💾 **System Management**: Disk usage overview and resource pruning
- ⌨️ **Keyboard-centric**: Vim-inspired keybindings for efficient navigation
- 🎨 **Clean UI**: Built with [Ratatui](https://github.com/ratatui/ratatui) for a modern terminal experience

## Installation

### Prerequisites

- Docker Engine (20.10+ recommended)
- For cargo installation: Rust toolchain (1.70+)

### Option 1: Quick Install (Recommended)

Install Contui with a single command using our standalone installer:

**macOS / Linux:**
```bash
curl -LsSf https://raw.githubusercontent.com/aycandv/contui/main/scripts/install.sh | sh
```

**Windows (PowerShell):**
```powershell
powershell -ExecutionPolicy ByPass -c "irm https://raw.githubusercontent.com/aycandv/contui/main/scripts/install.ps1 | iex"
```

The installer will:
- Detect your OS and architecture
- Download the latest release from GitHub
- Install to `~/.local/bin` (macOS/Linux) or `%USERPROFILE%\.local\bin` (Windows)

After installation, you may need to add the install directory to your PATH:
```bash
# Add to your shell profile (~/.bashrc, ~/.zshrc, etc.)
export PATH="$HOME/.local/bin:$PATH"
```

### Option 2: Install from Source

```bash
# Clone the repository
git clone https://github.com/aycandv/contui.git
cd contui

# Build and install
cargo install --path .

# Run
contui
```

### Option 3: Build from Source (Development)

```bash
# Clone the repository
git clone https://github.com/aycandv/contui.git
cd contui

# Build release binary
cargo build --release

# The binary will be at:
./target/release/contui
```

### Option 4: Using Cargo

Once published to crates.io:
```bash
cargo install contui
```

Or install directly from GitHub:
```bash
cargo install --git https://github.com/aycandv/contui
```

## Usage

### Starting Contui

```bash
# Basic usage
contui

# With custom Docker host
contui --host tcp://localhost:2375

# Enable debug logging
contui -v

# Show help
contui --help
```

### Navigation

| Key | Action |
|-----|--------|
| `1-6` | Switch between tabs (Containers, Images, Volumes, Networks, Compose, System) |
| `←/→` or `Tab` | Navigate between tabs |
| `j/k` or `↑/↓` | Navigate lists |
| `?` or `h` | Toggle help overlay |
| `q` or `Ctrl+C` | Quit |

### Containers Tab

| Key | Action |
|-----|--------|
| `s` | Start/Stop selected container |
| `r` | Restart container |
| `p` | Pause/Unpause container |
| `k` | Kill container |
| `d` | Delete container |
| `l` | View logs |
| `m` | Toggle stats panel |
| `i` | Inspect container details |
| `x` | Exec into container |
| `a` | Attach to the container's main process (detach with `Ctrl+P` `Ctrl+Q`; `Ctrl+C` reaches the process) |

### Images Tab

| Key | Action |
|-----|--------|
| `d` | Delete selected image |
| `p` | Prune dangling images |
| `i` | Inspect image details |

### Volumes & Networks Tabs

| Key | Action |
|-----|--------|
| `d` | Delete selected volume/network |
| `p` | Prune unused volumes/networks |

### System Tab

| Key | Action |
|-----|--------|
| `p` | Prune unused resources (opens dialog) |

Shows:
- Docker version and system info
- Disk usage breakdown (images, containers, volumes, build cache)
- Total reclaimable space

### Log Viewer

When viewing container logs (`l` key):

| Key | Action |
|-----|--------|
| `↑/↓` or `PgUp/PgDn` | Scroll |
| `r` | Refresh logs |
| `f` | Toggle follow mode (auto-scroll) |
| `/` | Search logs (regex supported) |
| `n/N` | Next/previous search match |
| `0-3` | Filter by level: 0=All, 1=Error, 2=Warn, 3=Info |
| `t/T` | Time filter: cycle/clear |
| `s` | Save logs to file |
| `Home/End` | Jump to top/bottom |
| `q/Esc` | Close log view |

### Exec Pane

When opening an exec shell (`x` key):

| Key | Action |
|-----|--------|
| `Ctrl+E` | Toggle focus between UI and exec |

#### Search in Logs

1. Press `/` to open search
2. Type your search pattern (regex supported)
3. Press `Enter` to search
4. Use `n` for next match, `N` for previous
5. Press `Esc` to clear search

#### Log Filters

- **Level Filter**: Press `0` (All), `1` (Error), `2` (Warn), `3` (Info)
- **Time Filter**: Press `t` to cycle through 5m, 15m, 1h, 24h

### Detail Viewer

When inspecting containers (`i` key) or images:

| Key | Action |
|-----|--------|
| `↑/↓` or `PgUp/PgDn` | Scroll |
| `Home/End` | Jump to top/bottom |
| `/` | Search (case-insensitive); type, then `Enter` to keep |
| `n/N` | Next/previous match |
| `Esc` | Clear the search, or close the detail view if there is none |
| `q` | Close detail view |

### Prune Dialog

In the System tab, press `p` to open the prune dialog:

1. Navigate options with `↑/↓`
2. Toggle checkboxes with `Space`
3. Press `Enter` to confirm and prune selected resources
4. Press `Esc` or `q` to cancel

**Available prune options:**
- Containers (stopped)
- Images (dangling/untagged)
- Volumes (unused)
- Networks (unused)
- Everything (select all)

## Configuration

Contui stores its configuration and logs in standard directories:

### Config Directory

- **Linux**: `~/.config/contui/`
- **macOS**: `~/Library/Application Support/com.contui.contui/`
- **Windows**: `%APPDATA%\contui\contui\config\`

### Theme

Create `config.toml` in the config directory and set the theme under `[ui]`:

```toml
[ui]
theme = "auto"     # default: follow the terminal's own colours (light or dark)
# theme = "classic"  # fixed palette: black background, white/gray text
```

`auto` uses your terminal's foreground and background plus bold, dim and
reverse video, and keeps green/red/yellow for status. `dark` is accepted as an
alias for `classic`.

### Podman

Contui also works with Podman through its Docker-compatible API socket. With
no `--host`, `--runtime auto` (the default) uses the docker socket if there is
one and otherwise `$XDG_RUNTIME_DIR/podman/podman.sock` or
`/run/podman/podman.sock`. Force an engine with `--runtime docker|podman` or
in `config.toml`:

```toml
[docker]
runtime = "podman"
```

The Podman service must be running, e.g. `systemctl --user enable --now
podman.socket` (rootless) or `systemctl enable --now podman.socket` (rootful).

### Log File

Logs are written to `/tmp/contui.log` for debugging purposes.

### Environment Variables

- `DOCKER_HOST`: Docker daemon socket (e.g., `unix:///var/run/docker.sock`)
- `RUST_LOG`: Log level (e.g., `contui=debug`)

## Docker Connection

Contui connects to Docker using the following priority:

1. `--host` CLI argument
2. `DOCKER_HOST` environment variable
3. Default Docker socket

### Connecting to Remote Docker

```bash
# Via TCP
contui --host tcp://192.168.1.100:2375

# Via SSH (needs the docker CLI on the remote host; uses your ssh keys and
# known_hosts, so connect once with `ssh user@remote-host` first)
contui --host ssh://user@remote-host
contui --host user@remote-host      # no scheme means ssh

# Set environment variable
export DOCKER_HOST=tcp://192.168.1.100:2375
contui
```

## Versioning & Releases

Contui uses [Semantic Versioning](https://semver.org/) (MAJOR.MINOR.PATCH):
- **MAJOR**: Breaking changes
- **MINOR**: New features (backward compatible)
- **PATCH**: Bug fixes

### Automatic Version Bump

You can manually trigger a version bump via GitHub Actions:

1. Go to **Actions** tab → **Version Bump**
2. Click **Run workflow**
3. Select bump type: `patch`, `minor`, or `major`
4. Click **Run workflow**

This will:
- Update `Cargo.toml` version
- Create a git commit
- Push the commit and create a new tag
- Trigger the release workflow automatically

### Conventional Commits (Optional)

For automatic semantic releases, use [Conventional Commits](https://www.conventionalcommits.org/):

```
feat: add new feature        → bumps MINOR
fix: fix a bug              → bumps PATCH
feat!: breaking change      → bumps MAJOR
```

## Building

### Requirements

- Rust 1.70 or newer
- Docker (for integration tests)

### Build Commands

```bash
# Debug build
cargo build

# Release build (optimized)
cargo build --release

# Run tests
cargo test

# Run with logging
RUST_LOG=contui=debug cargo run
```

### Development

```bash
# Watch mode (requires cargo-watch)
cargo watch -x run

# Check code
cargo clippy

# Format code
cargo fmt
```

## Troubleshooting

### "Cannot connect to Docker"

- Ensure Docker daemon is running: `docker ps`
- Check permissions: Add your user to the `docker` group
- Try with sudo (not recommended): `sudo contui`

### "Permission denied"

On Linux/macOS, add your user to the docker group:

```bash
sudo usermod -aG docker $USER
# Log out and log back in
```

### Logs not showing

- Check that the container has logs: `docker logs <container>`
- Ensure the container is running or has run previously
- Check `/tmp/contui.log` for errors

### Ghost text / display issues

Press `Ctrl+L` or switch tabs to refresh the display. This is a known terminal rendering issue.

## Architecture

Contui is built with Rust and uses:

- **[Ratatui](https://github.com/ratatui/ratatui)**: Terminal UI framework
- **[Bollard](https://github.com/fussybeaver/bollard)**: Docker API client
- **[Tokio](https://tokio.rs/)**: Async runtime
- **[Crossterm](https://github.com/crossterm-rs/crossterm)**: Cross-platform terminal handling

## Contributing

Contributions are welcome! Please feel free to submit a Pull Request.

1. Fork the repository
2. Create your feature branch (`git checkout -b feature/amazing-feature`)
3. Commit your changes (`git commit -m 'Add amazing feature'`)
4. Push to the branch (`git push origin feature/amazing-feature`)
5. Open a Pull Request

## License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.

## Acknowledgments

- Inspired by [lazydocker](https://github.com/jesseduffield/lazydocker) and [oxker](https://github.com/mrjackwills/oxker)
- Built with the excellent Rust ecosystem

---

Made with ❤️ and Rust
