# Development milestones

Ordered plan for major features. Earlier phases reduce rework in later ones.

---

## Milestone 1 — MPRIS (desktop media controls)

Expose playback through the standard D-Bus [MPRIS](https://specifications.freedesktop.org/mpris-spec/latest/) interface so desktop shells, widgets, and tools can see and control playterm.

- [x] Choose stack (e.g. `mpris-server`, `zbus`, or hand-rolled) and add Linux-only dependency gating if needed.
- [x] Implement `org.mpris.MediaPlayer2` (Raise, Quit, optional fullscreen) as appropriate for a TUI app.
- [x] Implement `org.mpris.MediaPlayer2.Player` with at least: `PlaybackStatus`, `Metadata` (title, artist, album, track id, length, art URL or local path if feasible), `Volume` if it maps cleanly to your engine.
- [x] Wire **Play / Pause / PlayPause / Stop / Next / Previous / Seek** to existing player commands.
- [x] Emit **PropertiesChanged** when track, state, or position should update (debounce if hot paths are noisy).
- [x] Document behavior and any gaps (e.g. no native window to “Raise”) in config or README.

---

## Milestone 2 — Library metadata cache + fzf

Local index for fast fuzzy picking without per-keystroke Subsonic calls. **Ballpark size:** on the order of **hundreds of bytes per track** for text fields only → **~10–100+ MB** for very large libraries (depends on field set and serialization); avoid storing art or audio in this index.

- [ ] Define **schema** (song id, title, artist, album, duration, album id, artist id, etc.) and **storage** path (e.g. under `~/.cache/playterm/` or `~/.local/share/playterm/`).
- [ ] Implement **full refresh** from Subsonic (pagination / `getIndexes` / `getMusicDirectory` or equivalent for your server targets).
- [ ] Implement **incremental or TTL refresh** strategy (startup, manual key, or background) so stale libraries recover without full rescans every launch.
- [ ] Build a **stable text line format** for fzf input (include hidden id column or use `tab`/`null`-separated metadata if you use `--with-nth`).
- [ ] Spawn **fzf** (or `sk`) with configurable command / theme; parse selection → resolve to **track id** (or album/artist if you extend later).
- [ ] On pick, **enqueue or play** using existing queue APIs; handle cancel / empty.
- [ ] Add **config knobs**: enable/disable fzf picker, binary path, keybinding, cache path, max age, force-refresh binding.

---

## Milestone 3 — Layout and display options

Customizable Now Playing chrome and queue row format; optional integration points for fzf visibility.

- [ ] **Config model** for toggles (spectrum, lyrics overlay entry, art, progress style, fzf-related hints if any).
- [ ] **Queue line template** — replace fixed `(num). (title) (artist) (length)` with a small template or ordered field list (document placeholders: `{n}`, `{title}`, `{artist}`, `{album}`, `{duration}`, etc.).
- [ ] **Now Playing layout** — blocks for “info block” position (e.g. top / bottom / side), wrapping, truncation rules consistent with terminal width.
- [ ] **Defaults** match current behavior; migration is “no config change” = today’s UI.
- [ ] Document all new keys in `config.toml` commentary or docs.

---

## Milestone 4 — Queue and browse QOL (album vs artist)

Faster “add a whole album” / “add whole artist” flows on top of existing browser and queue.

- [ ] From **album** context: add all tracks in album order to queue (append vs play-next policy — pick one default, optional modifier).
- [ ] From **artist** context: add all tracks (define order: album release date, then disc/track) or “all from this artist node” depending on API shape.
- [ ] Optional: connect to **fzf** milestones (e.g. multi-select album or “all from artist” from picker).
- [ ] Keybinds and/or menu actions; document in help overlay.
- [ ] add 'replace' option for queue, not just add, maybe also a 'add at beginning' and 'add at end'
- [ ] Fix album art no longer showing
- [ ] Fix playing old queue causes network error (after closing and reopening)

---

## Dependency summary

| Milestone | Depends on |
|-----------|------------|
| 1 MPRIS | — |
| 2 Cache + fzf | — (cache is internal to this milestone) |
| 3 Layout | 2 helpful so “show fzf” is real; 1 optional for metadata clarity |
| 4 QOL | 2 optional for picker-driven flows |

If **layout pain is urgent**, implement a **minimal** queue template slice from Milestone 3 early, then finish the rest of Milestone 3 after fzf lands.
