# playterm

A terminal music player for Subsonic-compatible servers. Built in Rust with [ratatui](https://github.com/ratatui/ratatui).

Streams from [Navidrome](https://www.navidrome.org/), [Subsonic](https://www.subsonic.org/), or any server implementing the Subsonic API. Renders album art via the Kitty graphics protocol — including inside tmux.

---

## Features

- **Three-column browser** — Artists → Albums → Tracks with lazy loading and persistent state across sessions
- **Kitty graphics album art** — full-size art on the Now Playing screen, thumbnail art strip on the Home tab. Works in tmux via Unicode placeholder mode
- **Gapless playback** — pre-buffers the next track ~10 seconds before the current one ends
- **Synced lyrics** — fetched from [LRCLib](https://lrclib.net), auto-scrolling with highlighted current line. Falls back to plain text
- **FFT spectrum visualizer** — 32-band braille-rendered analyzer at 30 fps
- **Offline track cache** — LRU-evicted local cache with background prefetch of the next 2 tracks in queue
- **Dynamic accent color** — extracted from album art, interpolated via OKLab color space on track changes
- **Playlist management** — browse, create, rename, delete playlists and add/remove tracks via the Subsonic API
- **Home tab** — recently played albums with art, recent tracks, and a "Rediscover" section surfacing artists you haven't listened to recently
- **Play history** — recorded locally, persisted across sessions
- **Search** — local filter across artists, albums, tracks, and queue
- **Configurable keybinds and theme** — remap any key, override any color via `config.toml`
- **Mouse support** — click to select browser items, seek on the progress bar, switch tabs
- **Queue shuffle / unshuffle** — `s` to shuffle, `Z` to restore original order
- **Clean shutdown** — state, history, and queue saved on quit. SIGHUP, SIGTERM, and SIGPIPE handled gracefully

## Screenshots

<p align="center">
  <img src="docs/screenshots/Now Playing.png" width="49%" />
  <img src="docs/screenshots/Home.png" width="49%" />
</p>
<p align="center">
  <img src="docs/screenshots/Browse.png" width="49%" />
  <img src="docs/screenshots/Lyrics.png" width="49%" />
</p>
<p align="center">
  <img src="docs/screenshots/Visualizer.png" width="49%" />
  <img src="docs/screenshots/Playlists.png" width="49%" />
</p>
<p align="center">
  <img src="docs/screenshots/Info.png" width="49%" />
</p>

## Requirements

- Rust toolchain (stable)
- A Subsonic-compatible server (Navidrome recommended)
- A terminal with Kitty graphics protocol support for album art (Ghostty, Kitty, WezTerm). Without it, everything works — you just won't see album art
- **Linux:** ALSA development headers (`libasound2-dev` on Debian/Ubuntu)

## Installation

```sh
cargo install playterm
```

Or build from source:

```sh
git clone https://github.com/youruser/playterm-app.git
cd playterm-app
cargo build --release
```

The binary is at `target/release/playterm`.

---

## Configuration

On first run, playterm creates a default config at:

```
~/.config/playterm/config.toml
```

At minimum, fill in your server credentials:

```toml
[server]
url = "https://your-navidrome-instance.example.com"
username = "your_username"
password = "your_password"
```

### Environment variable overrides

Environment variables take priority over the config file:

```sh
export SUBSONIC_URL="https://your-server.example.com"
export SUBSONIC_USER="admin"
export SUBSONIC_PASS="your_password"
```

### Other config sections

```toml
[player]
default_volume = 70       # 0–100
max_bit_rate = 0          # kbps, 0 = unlimited

[cache]
enabled = true
max_size_gb = 2

[ui]
lyrics = false            # show lyrics overlay on startup

[theme]
dynamic = true            # extract accent color from album art
# accent = "#ff8c00"
# background = "#1a1a1a"
# surface = "#161616"
# foreground = "#d4d0c8"
# dimmed = "#5a5858"
# border = "#252525"
# border_active = "#3a3a3a"

[keybinds]
# Remap any key. Examples:
# play_pause = "Space"
# next_track = ">"
# prev_track = "<"
# See the in-app help popup (i) for the full list.
```

### tmux

For album art and focus events to work inside tmux, add to `~/.tmux.conf`:

```
set -g allow-passthrough on
set -g focus-events on
```

---

## Keybinds

| Key           | Action                                |
| ------------- | ------------------------------------- |
| `1` `2` `3`   | Switch to Home / Browse / Now Playing |
| `Tab`         | Cycle tabs forward                    |
| `j` / `k`     | Navigate up/down                      |
| `h` / `l`     | Navigate columns / scroll album strip |
| `Enter`       | Select / play                         |
| `a`           | Add track to queue                    |
| `A`           | Add all visible tracks to queue       |
| `p` / `Space` | Play / pause                          |
| `n` / `N`     | Next / previous track                 |
| `x`           | Shuffle queue                         |
| `Z`           | Unshuffle (restore original order)    |
| `+` / `-`     | Volume up/down                        |
| `←` / `→`     | Seek ±10 seconds (Now Playing)        |
| `/`           | Search                                |
| `L`           | Toggle lyrics                         |
| `V`           | Toggle spectrum visualizer            |
| `P`           | Toggle playlist overlay (Browse tab)  |
| `>`           | Add track to playlist (Browse tab)    |
| `t`           | Toggle dynamic accent color           |
| `i`           | Keybind help                          |
| `q`           | Quit                                  |

All keybinds are remappable in `config.toml`. Press `i` in-app for the full reference.

---

## Architecture

playterm is a three-crate Cargo workspace: `playterm-subsonic` (API client), `playterm-player` (audio engine), and `playterm` (TUI binary). See [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md) for details.

---

## Data locations

| Path                                   | Contents                          |
| -------------------------------------- | --------------------------------- |
| `~/.config/playterm/config.toml`       | Configuration                     |
| `~/.config/playterm/state.json`        | Persisted browser state and queue |
| `~/.local/share/playterm/history.json` | Play history                      |
| `~/.cache/playterm/tracks/`            | Offline track cache               |

---

## Acknowledgements

- [ratatui](https://github.com/ratatui/ratatui) — terminal UI framework
- [rodio](https://github.com/RustAudio/rodio) — audio playback
- [Navidrome](https://www.navidrome.org/) — the music server this was built for
- [LRCLib](https://lrclib.net) — synced lyrics API
- [rmpc](https://github.com/mierak/rmpc) — navigation and Kitty art inspiration

## License

MIT
