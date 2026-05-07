# rustgram

Telegram channel and chat downloader written in Rust. Supports an interactive TUI and a conventional CLI suitable for scripts and automation.

## Features

- **Interactive TUI** — navigate chats, configure filters, and watch download progress in a full terminal UI
- **Conventional CLI** — scriptable subcommands for automation, skills, and pipelines
- **MTProto native** — connects directly to Telegram's API via [`grammers`](https://github.com/Lonami/grammers), no Bot API limitations
- **Full auth flow** — phone number + OTP + optional 2FA (TOTP/SRP)
- **Media filters** — download all media or filter by photo, video, document, or audio
- **Safe resume** — `--skip-existing` skips files already on disk
- **Persistent session** — logs in once, session is saved locally

## Installation

### macOS / Linux — shell installer

```bash
curl --proto '=https' --tlsv1.2 -LsSf https://github.com/tiomoreno/rustgram/releases/latest/download/rustgram-installer.sh | sh
```

### Windows — PowerShell installer

```powershell
powershell -c "irm https://github.com/tiomoreno/rustgram/releases/latest/download/rustgram-installer.ps1 | iex"
```

### Debian / Ubuntu — `.deb`

```bash
# x86_64
curl -LO https://github.com/tiomoreno/rustgram/releases/latest/download/rustgram_x86_64.deb
sudo dpkg -i rustgram_x86_64.deb

# arm64
curl -LO https://github.com/tiomoreno/rustgram/releases/latest/download/rustgram_aarch64.deb
sudo dpkg -i rustgram_aarch64.deb
```

### Fedora / RHEL / openSUSE — `.rpm`

```bash
# x86_64
sudo rpm -i https://github.com/tiomoreno/rustgram/releases/latest/download/rustgram_x86_64.rpm

# aarch64
sudo rpm -i https://github.com/tiomoreno/rustgram/releases/latest/download/rustgram_aarch64.rpm
```

### Via Cargo

```bash
cargo install --git https://github.com/tiomoreno/rustgram
```

### Build from source

```bash
git clone https://github.com/tiomoreno/rustgram
cd rustgram
cargo build --release
# binary at ./target/release/rustgram
```

## Setup

You need Telegram API credentials from [my.telegram.org](https://my.telegram.org):

1. Log in at [my.telegram.org](https://my.telegram.org)
2. Go to **API development tools**
3. Create an app and copy **App api_id** and **App api_hash**

Credentials are stored in `~/.config/rustgram/config.toml` after the first prompt, or set them as environment variables:

```bash
export TG_API_ID=12345678
export TG_API_HASH=abcdef1234567890abcdef1234567890
```

## Usage

### TUI (interactive)

```bash
rustgram tui
```

Navigate with arrow keys, `/` to filter, `Enter` to select, `t` to cycle media type, `Esc` to go back.

```
╭─ rustgram ──────────────────────────── chats ─╮
│ Filter: /rust_                                │
├───────────────────────────────────────────────┤
│ ▶ Channel  Rust Advanced + Beginner           │
│   Group    Rust Brasil                        │
│   Private  John Doe                           │
├───────────────────────────────────────────────┤
│  ↑↓: navigate   Enter: open   /: filter       │
╰───────────────────────────────────────────────╯
```

### CLI

#### Authenticate

```bash
rustgram login
```

Prompts for phone number, sends OTP, handles 2FA if enabled. Session is saved to `~/.config/rustgram/session.session`.

#### Logout

```bash
rustgram logout
```

Removes the saved session file.

#### List chats

```bash
rustgram chats
rustgram chats --filter rust
```

Prints all dialogs (private chats, groups, channels) with their numeric IDs.

```
TYPE                 ID            NAME
----------------------------------------------------------------------
Channel              -1001879988768  Rust Advanced + Beginner
Group                -1009876543210  Rust Brasil
Private              123456789       John Doe
```

#### Download media

```bash
# By username
rustgram download channelname

# By numeric ID (from `rustgram chats`)
rustgram download 1879988768

# Filter: only videos, custom output dir
rustgram download @channelname --media-type video --output ~/Videos/rust

# Limit scan to last 500 messages
rustgram download 1879988768 --limit 500

# Search filter (client-side text match)
rustgram download 1879988768 --query "lecture"
```

**Options:**

| Flag | Default | Description |
|---|---|---|
| `--output`, `-o` | `~/Downloads/rustgram/<chat>/` | Output directory |
| `--media-type`, `-t` | `all` | `all`, `photo`, `video`, `document`, `audio` |
| `--limit`, `-l` | unlimited | Max messages to scan |
| `--query`, `-q` | — | Client-side text filter |
| `--skip-existing` | `true` | Skip files already on disk |

## Configuration

`~/.config/rustgram/config.toml`:

```toml
api_id = 12345678
api_hash = "abcdef1234567890abcdef1234567890"
```

Session file: `~/.config/rustgram/session.session`

Both paths are resolved via the OS config directory (`$XDG_CONFIG_HOME` on Linux, `~/Library/Application Support` on macOS).

## Project structure

```
src/
├── main.rs              # CLI entry point (clap)
├── config.rs            # Credentials loading (env > file > prompt)
├── telegram.rs          # Client connection and session helpers
├── media.rs             # Shared media utilities (filter, filename, size)
├── commands/
│   ├── login.rs         # `rustgram login`
│   ├── logout.rs        # `rustgram logout`
│   ├── chats.rs         # `rustgram chats`
│   └── download.rs      # `rustgram download`
└── tui/
    ├── mod.rs           # Terminal setup and event loop
    ├── app.rs           # App state machine + async event handling
    └── ui.rs            # ratatui rendering (all screens)
```

## TUI key bindings

| Key | Action |
|---|---|
| `↑` / `↓` | Navigate list |
| `Enter` | Select / confirm |
| `/` | Activate filter (chat list) |
| `Esc` | Back / clear filter |
| `t` | Cycle media type (download config) |
| `d` or `Enter` | Start download (download config) |
| `Ctrl+C` | Quit from anywhere |
| `q` or `Esc` | Quit / back (when download finishes) |

## Architecture notes

- **Auth is synchronous in the TUI** — login API calls (`request_login_code`, `sign_in`, `check_password`) are awaited directly in the event loop. They complete in under a second, so the brief freeze is acceptable.
- **Downloads are async** — a `tokio::spawn` background task streams chunks via an `mpsc` channel; the UI polls `try_recv` on every frame (50 ms tick).
- **`Downloadable` is an enum** — `grammers 0.7` uses `Downloadable::Media(media)` (not a trait). Only `Photo`, `Document`, and `Sticker` variants carry a file location; other media types (`Geo`, `Poll`, `Contact`, etc.) are skipped before calling `iter_download` to avoid a panic.
- **Session** — stored via `grammers_client::session::Session::save_to_file`. The file is a binary blob managed entirely by grammers.

## Dependencies

| Crate | Role |
|---|---|
| `grammers-client` | Telegram MTProto client |
| `tokio` | Async runtime |
| `clap` | CLI argument parsing |
| `ratatui` | TUI rendering |
| `crossterm` | Terminal backend and keyboard events |
| `dialoguer` | Interactive prompts (CLI mode) |
| `indicatif` | Progress bars and spinners (CLI mode) |
| `serde` + `toml` | Config file serialization |
| `dirs` | OS-specific config/download directories |
| `anyhow` | Error handling |

## License

MIT
