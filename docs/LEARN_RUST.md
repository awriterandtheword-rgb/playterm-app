# Learn Rust Through playterm

> **You built this app. Now let's understand it.**

This guide teaches programming from scratch using the music player running on your
machine as the teaching material. Every concept has a real example pulled directly
from the playterm source code. No toy programs, no made-up examples.

**What you need to know before starting:**

- You can navigate the terminal (`cd`, `ls`, `cat`)
- You're comfortable editing text files
- You've run `cargo build` at least once

**What you do NOT need:**

- Any prior programming experience
- Knowledge of any language
- A computer science degree

If you can edit a Docker Compose file and understand what `ports: "4533:4533"` means,
you are ready for this guide.

---

## Table of Contents

- [Part 1: Before Rust — Programming Fundamentals](#part-1-before-rust--programming-fundamentals)
- [Part 2: Rust-Specific Concepts](#part-2-rust-specific-concepts)
- [Part 3: How playterm Actually Works](#part-3-how-playterm-actually-works)
- [Part 4: Guided Exercises](#part-4-guided-exercises)
- [Part 5: Where to Go Next](#part-5-where-to-go-next)

---

# Part 1: Before Rust — Programming Fundamentals

## Chapter 1: What Is Code? What Does the Compiler Do?

### What Is Code?

Code is instructions for a computer written in a language the computer can understand.
Think of it like a config file, but instead of just *describing* a state, it
*describes a sequence of actions*.

Your `docker-compose.yml` file says "this container should use this image, expose this
port, mount this volume." It describes a desired configuration. Code goes further — it
says "check if the user pressed a key; if it was 'j', move the selection down; if the
list is at the bottom, do nothing."

Config files are static. Code is alive.

### What Is a Compiler?

A compiler is a program that reads your code and translates it into machine code —
the binary instructions the CPU actually executes. Think of it like a translator:
you write instructions in Rust (which is human-readable), and the compiler produces
a binary file (which is CPU-readable).

The compiler also acts as a strict proofreader. Before it produces the binary, it
checks your code for a huge range of errors. This is why Rust programs often "just
work" once they compile — the compiler caught the bugs first.

### What Happens When You Run `cargo build`?

`cargo` is Rust's package manager and build tool. It's like `apt` combined with
`make`. When you run `cargo build`:

1. **Cargo reads `Cargo.toml`** — this is like `docker-compose.yml` for a Rust project.
   It lists the project's name, version, and all the external libraries ("crates")
   it depends on.

2. **Cargo downloads dependencies** — any external libraries not already cached are
   fetched. These live in `~/.cargo/registry/`.

3. **The Rust compiler (`rustc`) compiles each file** — it reads all the `.rs` source
   files, checks them, and produces a binary.

4. **The binary appears in `target/debug/` or `target/release/`** — this is the
   actual executable you run.

```
cargo build          → fast compile, debug binary   → target/debug/playterm
cargo build --release → slow compile, fast binary   → target/release/playterm
```

The `--release` flag tells the compiler to optimize aggressively. The debug binary is
larger and slower but contains extra information that helps with debugging.

**In playterm's case:**

```
~/projects/playterm-app/
├── Cargo.toml          ← workspace root ("I contain 3 sub-projects")
├── playterm-subsonic/
│   └── Cargo.toml      ← "I am the network client library"
├── playterm-player/
│   └── Cargo.toml      ← "I am the audio engine library"
└── playterm/
    └── Cargo.toml      ← "I am the binary that users run"
```

This multi-folder structure is called a **workspace**. Each subfolder is a **crate**
(Rust's word for a package). The workspace compiles all three together.

**Exercise:** Open `playterm/Cargo.toml` and find the line that lists `ratatui` as a
dependency. What version does playterm use?

---

## Chapter 2: Variables and Values

### What Is a Variable?

A variable is a named box that holds a value. It's the same concept as a shell
variable (`MY_VAR=hello`) or a TOML key (`default_volume = 70`), but in code.

In Rust, you create a variable with the keyword `let`:

```rust
let name = "Dark Side of the Moon";
let year = 1973;
```

The variable `name` now holds the text, and `year` holds the number. Any time you
write `name` later in the code, it means "the value I stored here."

### From playterm — a real example:

**File:** `playterm/src/ui/now_playing.rs`, line 141

```rust
let e = app.playback.elapsed.as_secs();
let elapsed_str = format!("{}:{:02}", e / 60, e % 60);
```

What this does:

- Line 1: Creates a variable `e` that holds the number of elapsed seconds as an
  integer. `app.playback.elapsed` is a time duration; `.as_secs()` converts it to
  a plain number like `183` (meaning 3 minutes and 3 seconds).

- Line 2: Creates a variable `elapsed_str` that holds a formatted string like
  `"3:03"`. The `format!` macro builds a string: `{}` is replaced by `e / 60`
  (minutes), and `{:02}` is replaced by `e % 60` (seconds), with zero-padding to
  two digits. This is the time shown in the progress bar.

### The `mut` keyword — variables that change

By default, a variable in Rust cannot be changed after it's set. This is like a
read-only config value — it's set once and stays that way. If you need to change it,
you must explicitly say so with `mut` (short for "mutable").

```rust
let score = 0;      // cannot be changed
let mut score = 0;  // can be changed
score = 5;          // this works now
```

**From playterm — a real example:**

**File:** `playterm/src/main.rs`, line 98

```rust
let mut last_rendered_art: Option<(String, Rect)> = None;
let mut art_displayed = false;
```

These are variables in the main event loop. `art_displayed` starts as `false` and
gets changed to `true` when album art is drawn on screen. It needs `mut` because its
value changes throughout the program's life.

Compare to line 97:

```rust
let mut last_tab = app.active_tab;
```

`last_tab` needs `mut` because the code updates it every frame to track which tab
was active on the previous frame — used to detect when the user switches tabs.

**Exercise:** Find line 64 in `playterm/src/main.rs`. A variable is created with
`let mut stdout`. What do you think `stdout` represents? (Hint: it's the terminal's
output pipe.)

---

## Chapter 3: Types

### What Is a Type?

Every value has a *type* — a description of what kind of data it is. You already
know types from config files: a port number like `4533` is different from a hostname
like `"192.168.68.122"`. You wouldn't put a hostname where a port number is expected.

Rust is very strict about types. The compiler knows the type of every value and
refuses to compile code that mixes them up. This prevents an enormous class of bugs.

### The Basic Types

| Type | What it holds | Example |
|------|--------------|---------|
| `String` | Text (variable length) | `"Dark Side of the Moon"` |
| `&str` | A reference to text (we'll explain `&` later) | `"Not playing"` |
| `u32` | A positive integer, 32 bits | `1973` |
| `u64` | A positive integer, 64 bits | `183000` (milliseconds) |
| `u8` | A positive integer, 8 bits (0–255) | `70` (volume) |
| `f32` | A decimal number, 32 bits | `0.7` (volume as fraction) |
| `f64` | A decimal number, 64 bits | `0.85` (color threshold) |
| `bool` | True or false | `true`, `false` |
| `usize` | Positive integer sized for memory (list indices) | `3` (third item) |

The `u` in `u32` means "unsigned" (no negative numbers). The number is how many
bits of storage it uses. Bigger = can hold larger numbers.

### From playterm — types in the Song struct

**File:** `playterm-subsonic/src/models.rs`, lines 47–70

```rust
pub struct Song {
    pub id: String,
    pub title: String,
    pub album: Option<String>,
    pub artist: Option<String>,
    pub track: Option<u32>,
    pub duration: Option<u32>,
    pub bit_rate: Option<u32>,
    pub size: Option<u64>,
    // ...
}
```

Every field in this struct has a type:
- `id: String` — the Navidrome ID for this song, like `"abc123"`. Always present.
- `title: String` — the song title. Always present.
- `duration: Option<u32>` — length in seconds, but *might not be known* (hence `Option`).
- `size: Option<u64>` — file size in bytes. A `u64` because files can be large.
- `bit_rate: Option<u32>` — bitrate in kbps. `u32` because 320 fits easily.

### Option<T> — "maybe has a value"

`Option<T>` is Rust's way of saying "this might have a value, or it might not."
It's like a Docker health check that might return a result or might time out.

You'll see `Option` everywhere in playterm because lots of data might not exist:
- A song might not have an album name
- The current track might not have a known duration
- The user might not have selected an artist yet

We'll cover Option in depth in Part 2.

### Vec<T> — a list of things

`Vec<T>` is a variable-length list (vector). The `T` is a placeholder for whatever
type the list contains.

**From playterm:**

```rust
pub songs: Vec<Song>      // the playback queue — a list of songs
pub artists: Vec<Artist>  // the artist list fetched from Navidrome
```

It's exactly like a list in a YAML config, but in code.

### Type annotations

You can explicitly tell Rust what type a variable is:

```rust
let volume: u8 = 70;
let ratio: f64 = 0.75;
```

But usually Rust can *infer* the type from context. If you write `let volume = 70u8`,
the `u8` suffix tells Rust the number is an 8-bit unsigned integer.

**Exercise:** Look at `playterm/src/config.rs` lines 129–145. Find the `Config`
struct. List three fields and their types. What do you think the types tell you about
valid values for each field?

---

## Chapter 4: Functions

### What Is a Function?

A function is a named, reusable block of code. You define it once with `fn`, give
it a name, and then call it by name to run the code inside.

Think of it like an alias in your shell config:

```bash
alias gs="git status"   # shell alias — call gs, runs git status
```

In code:

```rust
fn greet() {
    println!("Hello!");
}

greet();  // runs the code inside, prints "Hello!"
```

### Parameters — giving a function inputs

Functions can accept values. These are called **parameters**:

```rust
fn greet(name: String) {
    println!("Hello, {}!", name);
}

greet("Alice".to_string());  // prints "Hello, Alice!"
```

The `name: String` part means "this function expects one input, called `name`, and
it must be a `String`."

### Return values — getting results back

Functions can also return a value. The `->` symbol means "returns":

```rust
fn double(x: u32) -> u32 {
    x * 2  // no semicolon on the last line = this is the return value
}

let result = double(5);  // result is 10
```

### From playterm — a real function

**File:** `playterm/src/ui/now_playing.rs`, lines 76–86

```rust
fn format_quality(song: &playterm_subsonic::Song) -> Option<String> {
    let lossless = song.suffix.as_deref()
        .map(|s| matches!(s.to_lowercase().as_str(), "flac" | "wav" | "alac" | "ape" | "aiff"))
        .unwrap_or(false);

    if lossless {
        let fmt = song.suffix.as_deref().unwrap_or("").to_uppercase();
        return Some(fmt);
    }
    song.bit_rate.map(|br| format!("{}kbps", br))
}
```

Breaking this down:
- `fn format_quality(...)` — defines a function named `format_quality`
- `song: &playterm_subsonic::Song` — takes one input: a reference to a `Song` (the
  `&` means "borrowed reference" — we'll explain this in Part 2)
- `-> Option<String>` — returns an `Option<String>`: either `Some("FLAC")` or `None`
- Inside, it checks whether the song is lossless (FLAC, WAV, etc.)
- If lossless, returns `Some("FLAC")` (or whichever format)
- Otherwise returns the bitrate as `Some("320kbps")`, or `None` if unknown

This function is called in the now-playing bar. Its return value is the small text
under the artist name that says "FLAC" or "320kbps".

### From playterm — `pub fn` (public functions)

You'll notice many functions in playterm start with `pub fn` instead of just `fn`.
The `pub` keyword means "this function is usable from outside this file." It's like
making a port accessible vs. leaving it internal to a container.

**File:** `playterm/src/app.rs`, line 260

```rust
pub fn accent(&self) -> Color {
    self.theme.effective_accent(if self.theme.dynamic {
        Some(self.accent_current)
    } else {
        None
    })
}
```

This function belongs to the `App` struct (more on that soon). It returns the accent
color — orange by default, or whatever color was extracted from the album art.

**Exercise:** Find the function `map_search_key` in `playterm/src/main.rs` (around
line 301). How many parameters does it take? What does it return?

---

## Chapter 5: Structs — Data Bundled Together

### What Is a Struct?

A struct is a way to bundle related pieces of data together under one name. It's
like a Docker Compose service definition — several keys that together describe one
thing.

```yaml
# YAML (Docker Compose)
navidrome:
  image: deluan/navidrome
  ports:
    - "4533:4533"
  volumes:
    - /music:/music
```

```rust
// Rust equivalent as a struct
struct NavidromeService {
    image: String,
    port: u16,
    music_dir: String,
}
```

### From playterm — the Song struct

**File:** `playterm-subsonic/src/models.rs`, lines 47–70

```rust
pub struct Song {
    pub id: String,
    pub title: String,
    pub album: Option<String>,
    pub artist: Option<String>,
    pub album_id: Option<String>,
    pub artist_id: Option<String>,
    pub track: Option<u32>,
    pub disc_number: Option<u32>,
    pub year: Option<u32>,
    pub genre: Option<String>,
    pub cover_art: Option<String>,
    pub duration: Option<u32>,
    pub bit_rate: Option<u32>,
    pub content_type: Option<String>,
    pub suffix: Option<String>,
    pub size: Option<u64>,
    pub path: Option<String>,
    pub starred: Option<String>,
}
```

This struct holds everything playterm knows about a song. When Navidrome returns
data about a track, it arrives as JSON and gets decoded into a `Song` struct. From
then on, the program passes `Song` values around instead of raw JSON.

### From playterm — the App struct

**File:** `playterm/src/app.rs`, lines 108–185

The `App` struct is the heart of the program. It holds *everything* about the
current state of the application:

```rust
pub struct App {
    pub active_tab: Tab,         // which tab is showing (Browser or NowPlaying)
    pub browser_focus: BrowserColumn, // which column is highlighted (Artists/Albums/Tracks)
    pub library: LibraryState,   // all the artist/album/track data
    pub queue: QueueState,       // the playback queue
    pub playback: PlaybackState, // what's currently playing, elapsed time, paused?
    pub config: Config,          // settings from config.toml
    pub should_quit: bool,       // set to true when user presses 'q'
    // ... many more fields
}
```

Think of `App` as the "state file" of the program — every piece of information the
application needs to remember is stored here.

### Accessing struct fields

You access fields with a dot:

```rust
app.active_tab      // get the current tab
app.should_quit     // check if we should exit
app.playback.paused // access a nested field
```

This is exactly like `docker.container.status` in a hypothetical config language.

### Creating a struct

**File:** `playterm/src/app.rs`, lines 218–252

```rust
Ok(Self {
    active_tab: Tab::Browser,       // starts on the Browser tab
    browser_focus: BrowserColumn::Artists, // artists column highlighted
    library: LibraryState::default(), // empty library to start
    queue: QueueState::default(),   // empty queue
    playback: PlaybackState::default(),
    should_quit: false,             // don't quit yet
    // ...
})
```

`Self` here means "an instance of this struct" (in this case, `App`). You provide
a value for each field.

**Exercise:** Look at `playterm/src/state.rs` lines 72–83. Find the `QueueState`
struct. What field stores the list of songs? What field tracks which song is
currently playing?

---

## Chapter 6: Enums — A Value That Can Be One of Several Things

### What Is an Enum?

An enum (short for "enumeration") is a type that can be one of several named options.
You've seen this concept in config files — a service's restart policy can be `always`,
`unless-stopped`, or `no`. That's an enum.

```rust
enum RestartPolicy {
    Always,
    UnlessStopped,
    No,
}
```

### From playterm — the Tab enum

**File:** `playterm/src/app.rs`, lines 24–29

```rust
pub enum Tab {
    Browser,
    NowPlaying,
}
```

The application has exactly two tabs. Using an enum means the compiler guarantees
you'll never accidentally set `active_tab` to an invalid value — there's no third
option to mistype.

### From playterm — the Action enum

**File:** `playterm/src/action.rs`, lines 12–46

```rust
pub enum Action {
    Navigate(Direction),  // user pressed j/k/g/G
    Select,               // user pressed Enter
    Back,                 // user pressed Esc
    SwitchTab,            // user pressed Tab
    PlayPause,            // user pressed p or Space
    NextTrack,            // user pressed n
    PrevTrack,            // user pressed N
    VolumeUp,
    VolumeDown,
    Quit,
    None,                 // no action (key not recognized)
    // ... more
}
```

Every single thing the user can do is represented as one value in this enum. When
you press a key, the code figures out which `Action` it means, and that action is
passed to `app.dispatch()` which does the appropriate thing.

### Enums with data — variants that carry values

Some enum variants carry data with them. The `Navigate` variant above carries a
`Direction`:

```rust
pub enum Direction {
    Up,
    Down,
    Top,    // jump to first item (g key)
    Bottom, // jump to last item  (G key)
}
```

And `SearchInput` carries the character that was typed:

```rust
SearchInput(char),   // e.g. SearchInput('a'), SearchInput('b')
```

### From playterm — LoadingState

**File:** `playterm/src/state.rs`, lines 9–14

```rust
pub enum LoadingState<T> {
    NotLoaded,
    Loading,
    Loaded(T),
    Error(String),
}
```

This enum represents the four possible states of any data that gets fetched from
the network:

- `NotLoaded` — we haven't asked for it yet
- `Loading` — we sent the request, waiting for the response
- `Loaded(T)` — data arrived! The `T` is the actual data (e.g., `Vec<Artist>`)
- `Error(String)` — something went wrong; the string is the error message

When you open playterm, the artist list starts as `NotLoaded`, transitions to
`Loading` (shows "Loading…" in the UI), then becomes `Loaded(vec![...])` with all
your artists.

**Exercise:** Look at `playterm/src/engine.rs` lines 22–54. Find the `PlayerCommand`
and `PlayerEvent` enums. How many variants does `PlayerCommand` have? Which variant
carries a URL string?

---

## Chapter 7: If/Else — Making Decisions

### What Is If/Else?

If/else lets code take different paths based on a condition. It's like a shell
`if` statement:

```bash
if [ -f /etc/config ]; then
    echo "config exists"
else
    echo "no config"
fi
```

In Rust:

```rust
if condition {
    // run this if condition is true
} else {
    // run this if condition is false
}
```

### From playterm — deciding what to show in the controls bar

**File:** `playterm/src/ui/now_playing.rs`, lines 94–101

```rust
let (play_label, play_style) = if app.playback.current_song.is_none() {
    ("▶", Style::default().fg(t.dimmed))
} else if app.playback.paused {
    ("( ▶ )", Style::default().fg(app.accent()).add_modifier(Modifier::BOLD))
} else {
    ("( ⏸ )", Style::default().fg(app.accent()).add_modifier(Modifier::BOLD))
};
```

This decides what to show in the center of the now-playing bar:

1. If no song is loaded → show a dim grey `▶`
2. Else if the song is paused → show a bright orange `( ▶ )` (ready to resume)
3. Else (playing) → show a bright orange `( ⏸ )` (ready to pause)

Notice that in Rust, `if/else` can *return a value*. Here, the whole `if/else` block
returns a tuple `(label, style)` which is stored in `play_label` and `play_style`.
This is unique to Rust — your config-file intuition won't have this.

### From playterm — checking the tab

**File:** `playterm/src/main.rs`, lines 266–272

```rust
if kb.seek_forward.matches(code, modifiers) {
    return match active_tab {
        Tab::NowPlaying => Action::SeekForward,
        Tab::Browser    => Action::FocusRight,
    };
}
```

The right arrow key does different things depending on which tab is active. In the
NowPlaying tab it seeks forward 10 seconds; in the Browser tab it moves to the
next column. One key press, two behaviors — decided by an if check.

### From playterm — checking if art is displayed

**File:** `playterm/src/main.rs`, lines 135–159

```rust
if stored_matches && art_displayed {
    // Image is already visible — nothing to do.
} else if stored_matches && !art_displayed {
    // Same album, same rect — redisplay instantly.
    match ui::kitty_art::display_image(art_rect) {
        Ok(()) => art_displayed = true,
        Err(e) => eprintln!("kitty display: {e}"),
    }
} else {
    // Album changed or first display — full re-encode and re-transmit.
    match ui::kitty_art::render_image(bytes, art_rect) {
        Ok(()) => { /* ... */ }
        Err(e) => eprintln!("kitty render: {e}"),
    }
}
```

Three possible states for album art: already showing, cached but hidden, or needs
full re-render. If/else chains handle all three.

**Exercise:** Find the if/else in `playterm/src/ui/artists.rs` around line 44. What
does it show in the artist list if no artists match the search filter?

---

## Chapter 8: Match — Rust's Pattern Switch

### What Is Match?

`match` is like a more powerful version of if/else. It compares a value against
multiple patterns and runs the code for the first one that fits.

It's most useful with enums because you can handle each variant:

```rust
match app.active_tab {
    Tab::Browser    => { /* show browser */ }
    Tab::NowPlaying => { /* show now-playing */ }
}
```

The Rust compiler forces you to handle every possible variant. If you forget one,
it won't compile. This means you can never accidentally have code that doesn't handle
a case.

### From playterm — dispatching key presses

**File:** `playterm/src/main.rs`, lines 179–213

```rust
match event::read()? {
    Event::Key(key) => {
        if key.kind == KeyEventKind::Press {
            let action = if app.help_visible {
                map_help_key(key.code, key.modifiers, &app.keybinds)
            } else if app.search_mode.active {
                map_search_key(key.code)
            } else {
                map_key(key.code, key.modifiers, app.active_tab, &app.keybinds)
            };
            app.dispatch(action);
        }
    }
    Event::Mouse(mouse) => {
        if mouse.kind == MouseEventKind::Down(MouseButton::Left) {
            handle_mouse_click(mouse.column, mouse.row, app, area);
        }
    }
    Event::Resize(_, _) => {
        if app.kitty_supported && art_displayed {
            let _ = ui::kitty_art::clear_image();
        }
    }
    _ => {}
}
```

The event loop reads events from the terminal and `match`es on what arrived:
- A keyboard event? Map it to an action.
- A mouse event? Handle the click.
- A resize event? Clear the album art and re-render it at the new size.
- Anything else? `_ => {}` — the underscore is a catch-all that does nothing.

### From playterm — match on LoadingState

**File:** `playterm/src/ui/artists.rs`, lines 22–71

```rust
match &app.library.artists {
    LoadingState::NotLoaded | LoadingState::Loading => {
        let item = ListItem::new("Loading…").style(Style::default().fg(t.dimmed));
        // ... show loading spinner
    }
    LoadingState::Error(e) => {
        let item = ListItem::new(format!("Error: {e}")).style(...);
        // ... show error
    }
    LoadingState::Loaded(artists) => {
        // ... render the actual artist list
    }
}
```

One `match` handles all four possible states of the artist data. This is idiomatic
Rust — when you have an enum, you `match` on it.

### From playterm — match on search key input

**File:** `playterm/src/main.rs`, lines 301–309

```rust
fn map_search_key(code: KeyCode) -> Action {
    match code {
        KeyCode::Esc       => Action::SearchCancel,
        KeyCode::Enter     => Action::SearchConfirm,
        KeyCode::Backspace => Action::SearchBackspace,
        KeyCode::Char(ch)  => Action::SearchInput(ch),
        _                  => Action::None,
    }
}
```

When search mode is active, this function maps key codes to actions. `KeyCode::Char(ch)`
is a pattern that matches any character key — the `ch` variable captures which
character was pressed, then passes it into `SearchInput(ch)`.

**Exercise:** In `playterm/src/main.rs`, find the `map_key` function (line 237).
Find the `match` inside it (hint: look for `return match active_tab`). What two
actions can the right-arrow key trigger depending on the tab?

---

## Chapter 9: Loops — Doing Things Repeatedly

### What Is a Loop?

A loop runs a block of code multiple times. Programs are loops — they keep checking
for input and responding to it until you tell them to stop.

There are three kinds of loops in Rust:

```rust
loop {
    // runs forever until you 'break'
}

while condition {
    // runs while condition is true
}

for item in collection {
    // runs once for each item in the collection
}
```

### From playterm — the main event loop

**File:** `playterm/src/main.rs`, lines 102–224

The entire program's lifetime is one big `loop {}`:

```rust
loop {
    // 1. Drain library updates from background tasks
    while let Ok(update) = app.library_rx.try_recv() {
        app.apply_library_update(update);
    }
    // 2. Drain player events
    while let Ok(event) = app.player_rx.try_recv() {
        app.handle_player_event(event);
    }
    // 3. Draw the UI
    terminal.draw(|f| ui::render(app, f))?;
    // 4. Wait for an event (key, mouse, resize)
    if event::poll(Duration::from_millis(50))? {
        match event::read()? { /* ... */ }
    }
    // 5. Check if we should stop
    if app.should_quit {
        break;
    }
}
```

Every ~50ms, the loop:
1. Processes any network responses that arrived (new artist data, cover art, etc.)
2. Processes any audio events (progress updates, track ended, etc.)
3. Redraws the entire terminal UI
4. Checks if a key was pressed
5. Checks if the user pressed 'q'

When `should_quit` becomes `true`, `break` exits the loop and the program cleans up
and exits.

### From playterm — `while let` loops for draining channels

```rust
while let Ok(update) = app.library_rx.try_recv() {
    app.apply_library_update(update);
}
```

`while let` is a special loop that runs as long as a pattern matches. Here it keeps
calling `try_recv()` (check for a message without blocking) until there are no more
messages. This drains all pending updates before drawing the frame.

### From playterm — `for` loops over lists

**File:** `playterm/src/ui/artists.rs`, lines 47–49

```rust
let items: Vec<ListItem> = visible.iter()
    .map(|(_, name)| ListItem::new(*name).style(Style::default().fg(t.foreground)))
    .collect();
```

This iterates over every visible artist name and creates a `ListItem` for each one.
The `map` call transforms each artist name into a UI widget. `collect()` gathers
all the results into a `Vec`. We'll cover this more in Part 2.

**Exercise:** In `playterm/src/main.rs`, find the `for` loop that builds the queue
area (search for `for item in`). Alternatively, find the loop in `playterm/src/state.rs`
in the `QueueState::push` method. What does it do?

---

## Chapter 10: Printing and Formatting

### println! and eprintln!

Rust has two basic printing macros:

```rust
println!("Hello, world!");        // prints to standard output (the terminal)
eprintln!("warn: {e}");           // prints to standard error (usually also the terminal)
```

The `!` means it's a *macro*, not a regular function. Macros are special — they can
do things regular functions can't, like accept a format string.

### format! — building strings

`format!` works like `println!` but returns a `String` instead of printing it:

```rust
let s = format!("{}:{:02}", 3, 7);  // s = "3:07"
```

The `{}` placeholder is replaced by the first argument. `{:02}` means "zero-padded
to 2 digits." `{:.2}` means "2 decimal places." Rust's formatting system is very
powerful.

### From playterm — real formatting examples

**File:** `playterm/src/ui/now_playing.rs`, line 141

```rust
let elapsed_str = format!("{}:{:02}", e / 60, e % 60);
```

`e` is seconds elapsed (say 183). `e / 60 = 3` (minutes). `e % 60 = 3` (remaining
seconds). `{:02}` zero-pads seconds so you get `"3:03"` not `"3:3"`.

**File:** `playterm/src/ui/now_playing.rs`, line 85

```rust
song.bit_rate.map(|br| format!("{}kbps", br))
```

If `bit_rate` is `Some(320)`, this produces `"320kbps"`. We'll explain `.map()` in
Part 2.

### From playterm — eprintln! for warnings

**File:** `playterm/src/main.rs`, lines 48–50

```rust
if let Err(e) = persist::restore_state(&mut app) {
    eprintln!("warn: could not restore state: {e}");
}
```

`eprintln!` is used for warnings that shouldn't interrupt the user. They go to
stderr, so they appear before the TUI starts (or after it ends) rather than
corrupting the terminal display.

**Exercise:** Find `format_quality` in `playterm/src/ui/now_playing.rs`. What format
string does it use to produce "320kbps"? What does the `{}` placeholder fill in?

---

# Part 2: Rust-Specific Concepts

These are the things that make Rust different from other languages. They're harder
to grasp at first, but they're also what makes Rust so powerful and safe.

---

## Chapter 11: Ownership and Borrowing

### The Problem Rust Solves

In most programming languages, you can have multiple references to the same piece
of data. This causes bugs: two parts of the program modify the same data
simultaneously, and chaos ensues. Languages like Python and JavaScript use a "garbage
collector" to manage this, which adds runtime overhead. C and C++ leave it to you,
which leads to crashes.

Rust solves this at compile time with a system called **ownership**.

### Ownership — each value has exactly one owner

Every value in Rust has one owner. When the owner goes out of scope (when the
function finishes, or the block ends), the value is automatically freed.

```rust
{
    let song_title = String::from("Time");  // song_title owns this string
    // ... use song_title ...
}  // song_title goes out of scope, string is freed automatically
```

This is like being the sole admin of a server. When you leave (the scope ends),
the server shuts down.

### Borrowing — looking without owning

Often you want to let a function look at a value without giving it ownership. This
is a **borrow**, written with `&`:

```rust
fn print_title(song: &Song) {
    println!("{}", song.title);
    // We don't own song, so we can't free it
    // When this function ends, the Song stays alive in the caller
}

let my_song = Song { title: "Time".to_string(), ... };
print_title(&my_song);  // borrow my_song — still valid after this call
println!("{}", my_song.title);  // fine — we still own it
```

The `&` means "give me a reference to this, not ownership."

### From playterm — borrowing everywhere

**File:** `playterm/src/ui/artists.rs`, line 9

```rust
pub fn render(app: &App, frame: &mut Frame, area: Rect, is_active: bool) {
```

This function takes `app: &App` — a borrowed reference to the app state. The render
function can read everything in `App` but can't change it. This is safe: multiple
render functions can borrow `App` at the same time to draw different parts of the UI.

`frame: &mut Frame` is a *mutable borrow* — this function can modify the `Frame`
(drawing to the terminal), but only one function can have a mutable borrow at a time.

### From playterm — `Arc` for shared ownership

Some data in playterm is shared between multiple parts of the program:

**File:** `playterm/src/app.rs`, line 115

```rust
pub subsonic: Arc<SubsonicClient>,
```

`Arc` stands for "Atomically Reference Counted." It's a smart pointer that allows
multiple owners — but it counts the owners and only frees the data when the last
one is done. It's used for `SubsonicClient` because multiple background tasks need
to make API calls simultaneously.

**File:** `playterm/src/app.rs`, lines 305–313

```rust
pub fn fetch_artists(&self) {
    let client = self.subsonic.clone();  // clones the Arc, not the client
    let tx = self.library_tx.clone();
    tokio::spawn(async move {
        let result = playterm_subsonic::fetch_library(&client).await;
        // ...
    });
}
```

`self.subsonic.clone()` clones the `Arc` pointer — cheap! — so the background task
has its own reference. The actual `SubsonicClient` data isn't duplicated.

### The borrow checker error you will encounter

The most common beginner error in Rust is the borrow checker refusing to compile code:

```
error[E0502]: cannot borrow `app.cache` as mutable because it is also borrowed as immutable
```

This usually means you're trying to read from and modify the same data at the same
time. The fix is usually to restructure the code to separate the read and the write.
The compiler is preventing a real bug — embrace it.

**Exercise:** Look at `playterm/src/app.rs` around line 304 (`fetch_artists`). Why
does it call `self.subsonic.clone()` before the `tokio::spawn` block? What would
happen if it tried to use `self.subsonic` directly inside the spawn?

---

## Chapter 12: String vs &str

### Two String Types

Rust has two string types, and beginners always trip over this:

| Type | Description | Analogy |
|------|-------------|---------|
| `String` | Owned, heap-allocated, growable | A mutable file you own |
| `&str` | Borrowed reference to string data | A read-only view of a file |

```rust
let owned: String = String::from("hello");  // owns the memory
let borrowed: &str = "hello";               // points to static memory in the binary
```

Most of the time:
- Use `String` when you need to build or modify a string
- Use `&str` when you're just reading a string someone else owns

### Converting between them

```rust
let s: String = "hello".to_string();  // &str → String
let r: &str   = &s;                   // String → &str (borrow it)
```

### From playterm — String fields in structs

**File:** `playterm-subsonic/src/models.rs`, line 16

```rust
pub id: String,
pub name: String,
```

Artist IDs and names are `String` because the struct *owns* the data. When a `Song`
is passed around, it brings its own copy of the title string.

### From playterm — &str for static text

**File:** `playterm/src/ui/artists.rs`, line 24

```rust
let item = ListItem::new("Loading…").style(Style::default().fg(t.dimmed));
```

`"Loading…"` is a `&str` — a string literal baked into the binary. No allocation
needed, no cleanup needed. For text that never changes, `&str` is perfect.

### From playterm — `.as_deref()` and `.as_str()`

You'll see these conversions often:

**File:** `playterm/src/ui/now_playing.rs`, line 31

```rust
let artist = song.artist.as_deref().unwrap_or("Unknown Artist");
```

`song.artist` is `Option<String>` — an optional owned String. `.as_deref()` converts
it to `Option<&str>` — an optional reference. Then `.unwrap_or("Unknown Artist")`
gives a `&str` fallback. This avoids unnecessary copies.

**Exercise:** In `playterm/src/config.rs`, the `merge_env_overrides` function reads
environment variables. Why does it use `.to_string()` when assigning values to
`cfg.server.url`? What type is `cfg.server.url`?

---

## Chapter 13: Option<T> — Handling "Might Not Exist"

### The Problem

In many languages, the absence of a value is represented by `null`. `null` causes
a whole class of bugs ("NullPointerException") because you can forget to check for
it and the program crashes.

Rust has no `null`. Instead, it has `Option<T>`:

```rust
enum Option<T> {
    Some(T),  // a value exists
    None,     // no value
}
```

The compiler forces you to handle both cases. You cannot accidentally use an
`Option<String>` as if it were a `String`.

### From playterm — `current_song: Option<Song>`

**File:** `playterm/src/state.rs`, line 131

```rust
pub struct PlaybackState {
    pub current_song: Option<Song>,
    pub elapsed: Duration,
    pub total: Option<Duration>,
    pub paused: bool,
}
```

`current_song` is `Option<Song>` because there might not be a song playing. `total`
is `Option<Duration>` because Navidrome sometimes doesn't report track length.

When the player is idle, `current_song` is `None`. When playing, it's `Some(song)`.

### Handling Option — `if let`

The most common way to handle an Option:

**File:** `playterm/src/ui/now_playing.rs`, line 30

```rust
if let Some(song) = &app.playback.current_song {
    // song is now a &Song — we can use it
    let artist = song.artist.as_deref().unwrap_or("Unknown Artist");
    // ...
} else {
    // no song is playing
    vec![Line::from("Not playing")]
}
```

`if let Some(song) = ...` simultaneously checks "is there a value?" and unpacks it.
If `current_song` is `Some(a_song)`, then `song` gets bound to `a_song` and the
`if` block runs. If it's `None`, the `else` block runs.

### Handling Option — `unwrap_or`

**File:** `playterm/src/ui/now_playing.rs`, line 31

```rust
let artist = song.artist.as_deref().unwrap_or("Unknown Artist");
```

`.unwrap_or(default)` extracts the value if it's `Some`, or returns the default if
it's `None`. Clean and concise.

### Handling Option — `.map()`

**File:** `playterm/src/ui/now_playing.rs`, line 85

```rust
song.bit_rate.map(|br| format!("{}kbps", br))
```

`.map(|x| ...)` transforms a `Some(x)` into `Some(result)`, and leaves `None` as
`None`. If `bit_rate` is `Some(320)`, this produces `Some("320kbps")`. If `bit_rate`
is `None`, it produces `None`. No check needed.

### From playterm — chained Option handling

**File:** `playterm/src/state.rs`, lines 50–57

```rust
pub fn current_album(&self) -> Option<&Album> {
    let artist_id = self.current_artist().map(|a| a.id.as_str())?;
    if let Some(LoadingState::Loaded(albums)) = self.albums.get(artist_id) {
        self.selected_album.and_then(|i| albums.get(i))
    } else {
        None
    }
}
```

The `?` after `.map(...)` is the "early return on None" operator. If there's no
current artist, the function immediately returns `None`. Otherwise it continues.
This keeps deeply nested logic flat and readable.

**Exercise:** Find `selected_artist: Option<usize>` in `playterm/src/state.rs`.
Why is it `Option<usize>` instead of just `usize`? What would `usize` mean? What
does `None` mean in this context?

---

## Chapter 14: Result<T, E> — Handling Errors

### The Problem

Many operations can fail: reading a file, making a network request, parsing JSON.
Most languages handle this with exceptions — special flow control that jumps to
error-handling code. Rust doesn't have exceptions.

Instead, Rust uses `Result<T, E>`:

```rust
enum Result<T, E> {
    Ok(T),   // success, with a value of type T
    Err(E),  // failure, with an error of type E
}
```

Every function that might fail returns a `Result`. The compiler forces you to handle
both outcomes.

### From playterm — loading config

**File:** `playterm/src/config.rs`, line 151

```rust
pub fn load() -> Result<Self> {
    let config_path = config_file_path()?;

    if !config_path.exists() {
        create_default(&config_path)?;
    }

    let text = std::fs::read_to_string(&config_path)
        .with_context(|| format!("reading {}", config_path.display()))?;

    let mut file_cfg: FileConfig = toml::from_str(&text)
        .with_context(|| format!("parsing {}", config_path.display()))?;
    // ...
    Ok(Config { /* ... */ })
}
```

This function:
1. Gets the config file path (might fail if HOME isn't set)
2. Creates a default config if missing (might fail if directory can't be created)
3. Reads the file (might fail if permissions are wrong)
4. Parses the TOML (might fail if the format is invalid)
5. Returns `Ok(Config { ... })` on success

Every `?` at the end of a line means: "if this returns `Err(...)`, stop here and
return that error to the caller." It's early-exit on failure.

### From playterm — using Result at startup

**File:** `playterm/src/main.rs`, lines 38–41

```rust
let config = Config::load().unwrap_or_else(|e| {
    eprintln!("error: {e}");
    process::exit(1);
});
```

`Config::load()` returns a `Result`. `.unwrap_or_else(|e| ...)` handles the two
cases: if it's `Ok(config)`, we get `config`. If it's `Err(e)`, we run the closure
— print the error and exit the program.

### From playterm — the `?` operator

**File:** `playterm/src/persist.rs`, lines 57–62

```rust
pub fn save_state(app: &App) -> Result<()> {
    // ...
    let json = serde_json::to_string_pretty(&state)?;
    std::fs::write(&path, json)
        .with_context(|| format!("writing state to {}", path.display()))?;
    Ok(())
}
```

`serde_json::to_string_pretty(...)` might fail (unlikely, but theoretically possible).
The `?` propagates the error up. `std::fs::write(...)` might fail (disk full,
permissions). The `?` propagates that too. If everything works, `Ok(())` is returned
— `()` means "nothing" (the function succeeded but doesn't return a meaningful value).

### anyhow — making errors easier

playterm uses the `anyhow` crate to simplify error handling. Instead of defining
custom error types, `anyhow::Result` accepts any error type and includes rich context.
`.with_context(|| "...")` adds a human-readable message to any error.

**Exercise:** Look at `playterm/src/main.rs` line 37: `async fn main() -> Result<()>`.
Why does `main` return a `Result`? What happens if it returns `Err`?

---

## Chapter 15: Vec<T> and Iterators

### Vec<T> — the list type

`Vec<T>` is a growable list. `T` is the type of items in the list.

```rust
let mut songs: Vec<Song> = Vec::new();  // empty list
songs.push(song1);                       // add to end
songs.push(song2);
let first = &songs[0];                   // access by index
let count = songs.len();                 // how many items
```

### From playterm — the queue as a Vec

**File:** `playterm/src/state.rs`, line 74

```rust
pub struct QueueState {
    pub songs: Vec<Song>,
    pub cursor: usize,  // index of current song
    pub scroll: usize,  // scroll offset for display
}
```

The queue is a `Vec<Song>`. Adding a track to the queue (`push`), advancing to the
next track (`cursor += 1`), and clearing the queue (`songs.clear()`) are all
standard Vec operations.

### Iterators — transforming and filtering lists

Rust's iterator system is one of its most expressive features. Instead of writing
loops, you chain operations:

```rust
let names: Vec<&str> = artists
    .iter()               // turn the Vec into an iterator
    .filter(|a| a.name.contains("Beatles")) // keep only matching items
    .map(|a| a.name.as_str())              // transform each item
    .collect();           // gather results back into a Vec
```

### From playterm — building the visible artist list

**File:** `playterm/src/ui/artists.rs`, lines 35–50

```rust
let visible: Vec<(usize, &str)> = if let Some(q) = &app.search_filter {
    artists.iter().enumerate()
        .filter(|(_, a)| a.name.to_lowercase().contains(q.as_str()))
        .map(|(i, a)| (i, a.name.as_str()))
        .collect()
} else {
    artists.iter().enumerate().map(|(i, a)| (i, a.name.as_str())).collect()
};
```

When search is active, this:
1. `.iter()` — creates an iterator over the artist list
2. `.enumerate()` — adds an index to each item: `(0, artist0)`, `(1, artist1)`, ...
3. `.filter(...)` — keeps only artists whose name contains the search query
4. `.map(...)` — extracts the index and name as a tuple `(usize, &str)`
5. `.collect()` — gathers results into a `Vec`

This is far more readable than an equivalent loop would be.

### From playterm — finding an item's position

**File:** `playterm/src/ui/artists.rs`, lines 53–54

```rust
let sel = app.library.selected_artist
    .and_then(|s| visible.iter().position(|(i, _)| *i == s));
```

`.position(|x| ...)` finds the index of the first item where the condition is true.
This translates the "original index in the full list" to the "position in the
filtered visible list" — needed so the right item gets highlighted.

**Exercise:** Look at `playterm/src/state.rs`, the `QueueState::push` method. The
`pre_shuffle_order` field is used for unshuffle. Trace through the code — what
does `push` do to `pre_shuffle_order` when it's `Some`? When it's `None`?

---

## Chapter 16: Closures — The |x| Syntax

### What Is a Closure?

A closure is a mini-function you write inline. It's like a shell command substitution
but for values.

```rust
let double = |x| x * 2;   // closure that doubles a number
let result = double(5);     // result = 10
```

The `|x|` part is the parameter list (like `fn(x)` but without a name). What follows
is the body.

### Why Closures?

Closures are used heavily with iterators because they let you describe *what to do*
to each element without writing a full function:

```rust
let names: Vec<&str> = artists
    .iter()
    .map(|a| a.name.as_str())  // closure: given an artist, return its name
    .collect();
```

### Closures capture their environment

Unlike regular functions, closures can use variables from the surrounding scope:

```rust
let search = "Beatles".to_string();
let matching: Vec<_> = artists
    .iter()
    .filter(|a| a.name.contains(&search))  // `search` is captured from outside
    .collect();
```

### From playterm — closures in filter

**File:** `playterm/src/ui/artists.rs`, line 38

```rust
.filter(|(_, a)| a.name.to_lowercase().contains(q.as_str()))
```

The closure `|(_, a)| ...` takes a tuple. The `_` means "I don't care about the
first element (the index)." `a` is the artist. The closure returns `true` if the
artist's name contains the search query — the filter keeps matching items.

### From playterm — closures in tokio::spawn

**File:** `playterm/src/app.rs`, lines 307–313

```rust
tokio::spawn(async move {
    let result = playterm_subsonic::fetch_library(&client)
        .await
        .map(|lib| lib.artists)
        .map_err(|e| e.to_string());
    let _ = tx.send(LibraryUpdate::Artists(result)).await;
});
```

The `async move { ... }` is a closure that:
- Is async (can use `await`)
- `move`s the captured variables (`client`, `tx`) into the closure — the closure
  takes ownership so it can run independently on another task

`.map(|lib| lib.artists)` — if the result is `Ok(lib)`, transform it to `Ok(lib.artists)`.
`.map_err(|e| e.to_string())` — if it's `Err(e)`, transform the error to a `String`.

**Exercise:** Find `.unwrap_or_else(|e| { ... })` in `playterm/src/main.rs` line 38.
This is a closure. What does `|e|` represent? What does the closure body do with `e`?

---

## Chapter 17: Modules and `use` Statements

### What Is a Module?

A module is a namespace — a way to organize code into logical groups. In playterm,
each `.rs` file is a module.

**File:** `playterm/src/main.rs`, lines 1–12

```rust
mod action;    // declares that action.rs is a module
mod app;       // declares that app.rs is a module
mod cache;     // declares that cache.rs is a module
mod config;    // ...
mod history;
mod keybinds;
mod lyrics;
mod persist;
mod state;
mod theme;
mod ui;        // ui is a folder (ui/mod.rs is the entry point)
```

This is like listing the files in your `/etc/nginx/conf.d/` directory — each `mod`
declaration tells Rust to include that file.

### `use` — importing names

**File:** `playterm/src/main.rs`, lines 14–35

```rust
use std::io;
use std::process;
use std::time::Duration;

use anyhow::Result;
use crossterm::event::{
    self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind, KeyModifiers,
    MouseButton, MouseEventKind,
};
use ratatui::Terminal;

use action::{Action, Direction};
use app::{App, BrowserColumn, Tab};
```

`use` brings names into scope. Without it, you'd write `ratatui::Terminal` every time.
With `use ratatui::Terminal`, you can just write `Terminal`.

It's like adding a directory to your `$PATH` — once it's there, you can call commands
by name without the full path.

### The crate system — external dependencies

**File:** `playterm/Cargo.toml` (excerpt)

```toml
[dependencies]
ratatui   = "0.28"        # terminal UI framework
crossterm = "0.28"        # terminal input/output
tokio     = { version = "1", features = ["full"] }  # async runtime
serde     = { version = "1", features = ["derive"] } # serialization
```

These are external crates — other people's code downloaded from crates.io. `use`
statements bring their types and functions into scope.

### From playterm — `pub` visibility

**File:** `playterm/src/state.rs`, lines 25–37

```rust
pub struct LibraryState {
    pub artists: LoadingState<Vec<Artist>>,
    pub selected_artist: Option<usize>,
    pub albums: HashMap<String, LoadingState<Vec<Album>>>,
    pub selected_album: Option<usize>,
    pub tracks: HashMap<String, LoadingState<Vec<Song>>>,
    pub selected_track: Option<usize>,
}
```

`pub` on the struct means it can be used from other modules. `pub` on a field means
other modules can read and write it. Without `pub`, the struct and its fields are
private to the module they're defined in — like an internal port in Docker that
isn't exposed.

**Exercise:** Open `playterm/src/ui/mod.rs`. It declares `pub mod artists`. What
does making a module `pub` do? Who can use the `artists` module?

---

## Chapter 18: Traits — What `impl` Means

### What Is a Trait?

A trait is a set of behaviors that a type promises to implement. It's like a Docker
container's interface: "any container that exposes port 80 and responds to /health
is a web service, regardless of what's inside."

```rust
trait Greet {
    fn say_hello(&self) -> String;
}

struct Dog;
impl Greet for Dog {
    fn say_hello(&self) -> String {
        "Woof!".to_string()
    }
}
```

`impl Greet for Dog` means "here is how Dog fulfills the Greet contract."

### From playterm — `impl` blocks

Most of playterm's logic lives in `impl` blocks — code attached to a type:

**File:** `playterm/src/state.rs`, lines 84–124

```rust
impl QueueState {
    pub fn push(&mut self, song: Song) { ... }
    pub fn current(&self) -> Option<&Song> { ... }
    pub fn next(&mut self) -> bool { ... }
    pub fn prev(&mut self) -> bool { ... }
}
```

This adds four methods to `QueueState`. `self` refers to the `QueueState` instance.
`&mut self` means "the method can modify the QueueState."

### From playterm — derive macros for standard traits

**File:** `playterm/src/app.rs`, line 24

```rust
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Tab {
    #[default]
    Browser,
    NowPlaying,
}
```

`#[derive(...)]` automatically implements standard traits:

| Trait | What it gives you |
|-------|------------------|
| `Debug` | `{:?}` formatting for printing during debugging |
| `Clone` | `.clone()` to make a copy |
| `Copy` | automatic copying when assigned (for small types) |
| `PartialEq, Eq` | `==` comparison |
| `Default` | `Tab::default()` returns `Tab::Browser` (the `#[default]` variant) |
| `Serialize, Deserialize` | read/write as JSON or TOML (from the `serde` crate) |

Deriving these saves you from writing dozens of lines of boilerplate.

### From playterm — trait objects

**File:** `playterm/src/lyrics.rs`, line 31

```rust
async fn fetch_inner(...) -> Result<Vec<LyricLine>, Box<dyn std::error::Error + Send + Sync>>
```

`dyn std::error::Error` is a *trait object* — "any type that implements the Error
trait." The `Box<...>` puts it on the heap because the compiler doesn't know its
size at compile time. `Send + Sync` are marker traits that say "this can be sent
across threads safely."

**Exercise:** Find `impl Tab` in `playterm/src/app.rs` (around line 31). What method
does it implement? What does that method do?

---

## Chapter 19: Async/Await — Doing Multiple Things at Once

### The Problem

Some operations take time: network requests, disk reads, waiting for user input.
If your program stops and waits for each one, the UI freezes.

playterm solves this with async/await — a way to write code that "pauses" while
waiting and lets other code run in the meantime, all without creating extra threads.

### `async fn` and `.await`

A function marked `async` can pause at `.await` points:

```rust
async fn fetch_artists(&self) {
    let result = self.subsonic.get_artists().await;  // pause here while waiting
    // ... continue once data arrives
}
```

While `get_artists()` is waiting for the network, the Tokio runtime runs other tasks
(handling key presses, processing player events, etc.).

### From playterm — spawning background tasks

**File:** `playterm/src/app.rs`, lines 303–313

```rust
pub fn fetch_artists(&self) {
    let client = self.subsonic.clone();
    let tx = self.library_tx.clone();
    tokio::spawn(async move {
        let result = playterm_subsonic::fetch_library(&client)
            .await
            .map(|lib| lib.artists)
            .map_err(|e| e.to_string());
        let _ = tx.send(LibraryUpdate::Artists(result)).await;
    });
}
```

`tokio::spawn` launches a background task. The task fetches artists from Navidrome
and sends the result back via a channel (`tx.send`). The main loop (the `while let
Ok(update) = app.library_rx.try_recv()` part) picks up the result on the next
iteration and updates the UI.

### Channels — communication between async tasks and threads

playterm has two communication channels:

1. **`library_tx` / `library_rx`** — between the main loop and background async tasks.
   Background tasks send `LibraryUpdate` messages (artist data, album art, lyrics).
   The main loop receives them with `library_rx.try_recv()`.

2. **`player_tx` / `player_rx`** — between the main loop and the audio thread.
   The main loop sends `PlayerCommand` (play, pause, seek).
   The audio thread sends back `PlayerEvent` (progress, track ended, error).

This channel pattern means the UI never blocks waiting for audio or network.

---

# Part 3: How playterm Actually Works

## The Full Journey: From `cargo run` to Music

Let's follow the program from the moment you type `cargo run` to the moment a song
starts playing.

---

### Step 1: `main.rs` runs — the entry point

**File:** `playterm/src/main.rs`, lines 37–84

```rust
#[tokio::main]
async fn main() -> Result<()> {
    let config = Config::load().unwrap_or_else(|e| {
        eprintln!("error: {e}");
        process::exit(1);
    });
    let mut app = App::new(config)?;

    app.kitty_supported = ui::kitty_art::detect_kitty_support();

    if let Err(e) = persist::restore_state(&mut app) {
        eprintln!("warn: could not restore state: {e}");
    }

    match history::PlayHistory::load(&history_path) {
        Ok(h) => app.history = h,
        Err(e) => eprintln!("warn: could not load history: {e}"),
    }

    app.fetch_artists();

    enable_raw_mode()?;
    // ...
    let result = run_loop(&mut terminal, &mut app).await;
    // ...
}
```

This happens in order:

1. Load config from `~/.config/playterm/config.toml` — fail fast if misconfigured
2. Create the `App` struct — initializes all state, spawns the audio thread
3. Detect Kitty terminal support (sends a probe character, checks the response)
4. Restore last session state from `~/.config/playterm/state.json`
5. Load play history
6. Fire off an async task to fetch the artist list from Navidrome
7. Enter "raw mode" — the terminal stops line-buffering and sends every key immediately
8. Enter the main event loop

---

### Step 2: Config loads — TOML becomes a Rust struct

**File:** `playterm/src/config.rs`

```
~/.config/playterm/config.toml
   ↓
std::fs::read_to_string(&path)   ← reads it as a String
   ↓
toml::from_str(&text)            ← parses the TOML into a FileConfig struct
   ↓
merge_env_overrides(&mut cfg)    ← overrides with TERMUSIC_SUBSONIC_* env vars
   ↓
Config { subsonic_url, ... }     ← the final runtime config
```

The `serde` library does the TOML-to-struct conversion. The `#[derive(Deserialize)]`
on `FileConfig` generates code that knows how to read each TOML key into the right
struct field.

**File:** `playterm/src/config.rs`, lines 100–108

```rust
#[derive(Debug, Serialize, Deserialize, Default)]
struct ServerSection {
    #[serde(default)]
    url: String,
    #[serde(default)]
    username: String,
    #[serde(default)]
    password: String,
}
```

The TOML section `[server]` maps to the `ServerSection` struct. The field names
match the TOML keys. `#[serde(default)]` means "use the default value if the key
is missing" — prevents a crash when the field isn't in the file.

---

### Step 3: The TUI starts — what is ratatui doing?

`ratatui` is a library for building terminal user interfaces. It handles the low-level
work of writing characters to the right positions on screen.

**File:** `playterm/src/main.rs`, lines 63–69

```rust
enable_raw_mode()?;
let mut stdout = io::stdout();
stdout.execute(EnterAlternateScreen)?;
stdout.execute(EnableMouseCapture)?;
let backend = CrosstermBackend::new(stdout);
let mut terminal = Terminal::new(backend)?;
```

Step by step:
1. `enable_raw_mode()` — terminal sends each keypress immediately (no buffering)
2. `EnterAlternateScreen` — switches to a blank screen, like `clear` but saved/restored
3. `EnableMouseCapture` — terminal sends mouse click events
4. `CrosstermBackend` — ratatui's bridge to the crossterm library
5. `Terminal::new(backend)` — the ratatui terminal object

When playterm exits, it reverses all of this (lines 78–83) so your terminal returns
to normal.

### The render cycle

Every ~50ms, the main loop calls:

```rust
terminal.draw(|f| ui::render(app, f))?;
```

`terminal.draw` takes a closure that receives a `Frame` (`f`). The frame is a
canvas for that single render cycle. `ui::render(app, f)` draws the entire UI onto
the frame. When the closure returns, ratatui computes the *diff* from the previous
frame and sends only the changes to the terminal — efficient!

**File:** `playterm/src/ui/mod.rs`, lines 22–41

```rust
pub fn render(app: &App, frame: &mut Frame) {
    match app.active_tab {
        Tab::Browser => {
            let areas = layout::build_browser(frame.area());
            browser::render(app, frame, areas.center);
            now_playing::render(app, frame, areas.now_playing);
            status_bar::render(app, frame, areas.status_bar);
        }
        Tab::NowPlaying => {
            let areas = layout::build_nowplaying(frame.area());
            nowplaying_tab::render(app, frame, areas.center);
            now_playing::render(app, frame, areas.now_playing);
            status_bar::render(app, frame, areas.status_bar);
        }
    }
    if app.help_visible {
        popup::render_help(app, frame);
    }
}
```

The `render` function:
1. Checks which tab is active
2. Builds the layout (divides the terminal into areas — top bar, center, bottom bar)
3. Renders each section into its area
4. If the help popup is open, renders it last (it layers on top)

---

### Step 4: The event loop — how pressing 'j' becomes "scroll down"

**File:** `playterm/src/main.rs`, lines 102–224

The main loop structure:

```
loop {
    1. Drain library updates  (network responses arrived)
    2. Drain player events    (audio progress, track ended)
    3. Advance color transition (smooth accent animation)
    4. Draw the UI
    5. Render Kitty album art (after ratatui, so it sits on top)
    6. Poll for events (50ms timeout)
       - Key press  → map to Action → app.dispatch(action)
       - Mouse click → handle_mouse_click(...)
       - Resize     → clear album art, re-render on next frame
    7. Drain player events again (in case a track started during render)
    8. Check should_quit
}
```

When you press 'j':
1. `event::read()` returns `Event::Key(key)` where `key.code == KeyCode::Char('j')`
2. `map_key(key.code, key.modifiers, app.active_tab, &app.keybinds)` looks up what
   'j' means → returns `Action::Navigate(Direction::Down)`
3. `app.dispatch(Action::Navigate(Direction::Down))` runs the scroll-down logic

---

### Step 5: The Action enum — how user input becomes behavior

Every possible user interaction is one variant of the `Action` enum. The `dispatch`
function in `app.rs` handles every action:

**File:** `playterm/src/app.rs` (dispatch function, simplified)

```rust
pub fn dispatch(&mut self, action: Action) {
    match action {
        Action::Navigate(Direction::Down) => {
            // increment the selected index in the active column
        }
        Action::PlayPause => {
            if self.playback.paused {
                let _ = self.player_tx.send(PlayerCommand::Resume);
                self.playback.paused = false;
            } else {
                let _ = self.player_tx.send(PlayerCommand::Pause);
                self.playback.paused = true;
            }
        }
        Action::Quit => {
            self.should_quit = true;
        }
        // ... 30+ more cases
        Action::None => {}
    }
}
```

The `dispatch` function is the brain of the application. Every action flows through
here. The `match` guarantees every action is handled.

---

### Step 6: The audio engine — how music actually plays

**File:** `playterm-player/src/engine.rs`

The audio engine runs on its own thread (not async — a real OS thread):

```rust
pub fn spawn_player() -> (mpsc::Sender<PlayerCommand>, mpsc::Receiver<PlayerEvent>) {
    let (cmd_tx, cmd_rx) = mpsc::channel::<PlayerCommand>();
    let (evt_tx, evt_rx) = mpsc::channel::<PlayerEvent>();

    std::thread::Builder::new()
        .name("playterm-player".into())
        .spawn(move || player_thread(cmd_rx, evt_tx))
        .expect("failed to spawn player thread");

    (cmd_tx, evt_rx)
}
```

The main loop gets `cmd_tx` (to send commands) and `evt_rx` (to receive events).
The player thread has `cmd_rx` (to receive commands) and `evt_tx` (to send events).

Inside the player thread:

```
loop:
  1. Drain all pending commands (PlayUrl, Pause, Resume, Seek, ...)
  2. If playing: compute elapsed, check for gapless transition, send Progress event
  3. If player became empty: send TrackEnded event
  4. Sleep 500ms, repeat
```

Why a thread instead of async? `rodio` (the audio library) uses blocking I/O. It
needs a real thread, not an async task.

---

### Step 7: Channels — how PlayerCommand and PlayerEvent work

Think of channels like a Unix pipe, but for Rust values:

```
Main thread    ──── PlayerCommand ────►  Audio thread
               ◄─── PlayerEvent   ────
```

**Sending a command** (main thread):

```rust
let _ = self.player_tx.send(PlayerCommand::PlayUrl {
    url: stream_url,
    duration: Some(Duration::from_secs(180)),
    gen: self.play_gen,
});
```

**Receiving the command** (audio thread):

```rust
match cmd_rx.try_recv() {
    Ok(PlayerCommand::PlayUrl { url, duration, gen }) => {
        play_url(url, duration, gen, ...);
    }
    Ok(PlayerCommand::Pause) => { player.pause(); }
    // ...
}
```

**Sending an event** (audio thread):

```rust
let _ = evt_tx.send(PlayerEvent::Progress {
    elapsed: player.get_pos(),
    total: current_total,
});
```

**Receiving the event** (main thread):

```rust
while let Ok(event) = app.player_rx.try_recv() {
    app.handle_player_event(event);
}
```

In `handle_player_event`:

```rust
PlayerEvent::Progress { elapsed, total } => {
    self.playback.elapsed = elapsed;
    if let Some(t) = total { self.playback.total = Some(t); }
}
PlayerEvent::TrackEnded => {
    if self.queue.next() {
        self.play_current();  // advance queue and play next song
    }
}
```

---

### Step 8: The Subsonic client — how playterm talks to Navidrome

**File:** `playterm-subsonic/src/client.rs`

The Subsonic API uses HTTP with MD5 authentication. Every request needs five
parameters:

```
u = username
t = MD5(password + salt)
s = random salt
v = 1.16.1
c = playterm
```

**File:** `playterm-subsonic/src/client.rs`, lines 39–41

```rust
fn make_token(password: &str, salt: &str) -> String {
    hex::encode(md5::compute(format!("{password}{salt}")).as_ref())
}
```

This hashes the password with a random salt to produce an authentication token.
The salt changes every request, so the token is different every time — even for
the same password.

To fetch the artist list:

1. playterm calls `client.get_artists()`
2. Which builds the URL: `http://192.168.68.122:4533/rest/getArtists.view?u=admin&t=abc&s=xyz&v=1.16.1&c=playterm&f=json`
3. Makes an HTTP GET request via `reqwest`
4. Gets back JSON
5. Deserializes the JSON into `Artists` / `Vec<Artist>` structs via `serde`
6. Returns `Ok(artists)` or `Err(...)`

The `stream_url` function constructs a URL for audio streaming:

```rust
pub fn stream_url(&self, id: &str, max_bit_rate: u32) -> String {
    let salt = random_ascii(8);
    let token = make_token(&self.password, &salt);
    format!(
        "{}/rest/stream.view?id={}&u={}&t={}&s={}&v={}&c={}&f=json&maxBitRate={}",
        self.base_url, id, self.username, token, salt, API_VERSION, CLIENT_NAME, max_bit_rate
    )
}
```

This URL is what gets sent to the audio engine as `PlayerCommand::PlayUrl { url }`.
The engine opens an HTTP connection to this URL and streams the audio data.

---

### Step 9: Album art — Kitty escape codes

**File:** `playterm/src/ui/kitty_art.rs`

The Kitty terminal graphics protocol lets you display actual images in the terminal
by sending specially crafted escape codes.

The process:

1. **Fetch:** `client.get_cover_art(cover_id)` makes an HTTP request to
   `getCoverArt.view` and returns raw JPEG/PNG bytes.

2. **Decode and resize:** The `image` crate decodes the image data and resizes it
   to fit the art area (computed in cells × pixels per cell).

3. **Encode:** Convert to RGBA8 pixels, compress with zlib, encode as base64.

4. **Transmit:** Write a Kitty escape sequence to the terminal:
   ```
   \x1b_Ga=T,f=32,o=z,s=W,v=H,m=0;<base64data>\x1b\\
   ```
   - `a=T` — transmit and display
   - `f=32` — pixel format: RGBA8
   - `o=z` — data is zlib-compressed
   - `s=W,v=H` — width and height in pixels

5. **Re-transmit on tab switch/resize:** When you switch to the Browser tab, the
   Kitty image is cleared (but stays in terminal memory). When you switch back,
   it's redisplayed instantly with `a=p,i=1` (place existing image).

This is why playterm only works in Kitty-compatible terminals (Kitty itself, WezTerm).
It does nothing in terminals that don't support the protocol.

---

### Step 10: State persistence — how quit/restart remembers everything

**File:** `playterm/src/persist.rs`

When you press 'q', the last thing the main loop does before exiting is:

```rust
if let Err(e) = persist::save_state(app) {
    eprintln!("warn: could not save state: {e}");
}
```

`save_state` serializes a `SavedState` struct to JSON:

```rust
let state = SavedState {
    active_tab: app.active_tab,
    browser_focus: app.browser_focus,
    selected_artist: app.library.selected_artist,
    selected_album: app.library.selected_album,
    selected_track: app.library.selected_track,
    queue: app.queue.songs.clone(),
    queue_cursor: app.queue.cursor,
};
let json = serde_json::to_string_pretty(&state)?;
std::fs::write(&path, json)?;
```

`serde_json::to_string_pretty` converts the Rust struct to formatted JSON.
The file is written to `~/.config/playterm/state.json`.

On next startup, `restore_state` reads that JSON and populates the `App`:

```rust
let text = std::fs::read_to_string(&path)?;
let state: SavedState = serde_json::from_str(&text)?;

app.active_tab = state.active_tab;
app.queue.songs = state.queue;
app.queue.cursor = state.queue_cursor;
// ...
```

The `#[derive(Serialize, Deserialize)]` on the structs and enums does all the
serialization work. You can see the file yourself:

```bash
cat ~/.config/playterm/state.json
```

---

# Part 4: Guided Exercises

These exercises start trivially easy and build up to making real changes to playterm.
Each one teaches a concept by having you touch the real code.

---

## Exercise 1: Change the Status Bar Hint Text

**Goal:** Change the `"i — help"` hint at the bottom right to something custom.

**Difficulty:** ⭐ (trivial)

**File:** `playterm/src/ui/status_bar.rs`, line 34

**What to change:**

```rust
let hint = "i — help";
```

Change it to:

```rust
let hint = "i — keybinds";
```

**Verify:**

```bash
cargo build
./target/debug/playterm
```

Look at the bottom right of the screen. The hint should say "i — keybinds".

**What you learned:** String literals in Rust are written with double quotes. You
can change any UI text by finding where it's defined and editing the string.

---

## Exercise 2: Change the Default Volume

**Goal:** Change the default volume from 70% to 80%.

**Difficulty:** ⭐ (trivial)

**File:** `playterm/src/config.rs`, line 124

```rust
fn default_volume() -> u8 { 70 }
```

Change it to:

```rust
fn default_volume() -> u8 { 80 }
```

**Also update the default config file template** (line 216 in the same file):

```toml
default_volume = 70
```

Change to:

```toml
default_volume = 80
```

**Verify:**

Delete or rename your existing config to force regeneration:

```bash
mv ~/.config/playterm/config.toml ~/.config/playterm/config.toml.bak
cargo build && ./target/debug/playterm
```

The player should start at 80% volume.

**What you learned:** Default values for config fields are defined as functions.
The `u8` type holds numbers 0–255, which maps neatly to a 0–100 volume percentage.

---

## Exercise 3: Change the Accent Color

**Goal:** Change the default accent color from orange to a different color.

**Difficulty:** ⭐⭐ (easy)

**File:** `playterm/src/theme.rs`, line 35

```rust
accent: p(sec.accent.as_deref(), Color::Rgb(255, 140, 0)),
```

Change the `Color::Rgb(255, 140, 0)` to your preferred color. RGB values are
0–255 for Red, Green, Blue:

```rust
// Blue
accent: p(sec.accent.as_deref(), Color::Rgb(100, 149, 237)),

// Teal
accent: p(sec.accent.as_deref(), Color::Rgb(0, 180, 160)),

// Pink
accent: p(sec.accent.as_deref(), Color::Rgb(255, 100, 150)),
```

**Verify:**

```bash
cargo build && ./target/debug/playterm
```

The highlighted items, active borders, and progress bar should now use your color.

**Note:** This only changes the *fallback* default. If the `[theme] dynamic = true`
setting extracts an accent from album art, that will override your default. To see
your static color, disable dynamic mode with `t` while the app is running.

**What you learned:** `Color::Rgb(r, g, b)` creates a color from red, green, blue
components. You can find good RGB values at any color picker website.

---

## Exercise 4: Change a Default Keybind

**Goal:** Change the "add track to queue" key from `a` to `e`.

**Difficulty:** ⭐⭐ (easy)

**File:** `playterm/src/keybinds.rs`, line 97

```rust
add_track: resolve(sec.add_track.as_deref(), KeySpec::new(KeyCode::Char('a'))),
```

Change `'a'` to `'e'`:

```rust
add_track: resolve(sec.add_track.as_deref(), KeySpec::new(KeyCode::Char('e'))),
```

**Verify:**

```bash
cargo build && ./target/debug/playterm
```

Press `e` on a track in the browser. It should add to the queue. Press `a` — it
should do nothing.

**Also update the help popup** so the popup shows the right key. Find `"a"` in
`playterm/src/ui/popup.rs` and update it. (You'll need to `grep -n '"a"' playterm/src/ui/popup.rs`
to find the exact line.)

**What you learned:** Default keybinds are `KeySpec` values. `KeyCode::Char('e')`
represents the letter 'e'. Special keys use variants like `KeyCode::Enter`,
`KeyCode::Tab`, `KeyCode::Left`.

---

## Exercise 5: Add a New Entry to the Help Popup

**Goal:** Add a new row to the keybind reference popup for the `t` key (toggle
dynamic theme).

**Difficulty:** ⭐⭐ (easy)

**File:** `playterm/src/ui/popup.rs`

Search for the "Volume & Display" section in the popup. It will contain rows like:

```rust
("t", "Toggle dynamic accent colour"),
```

If that entry already exists, find the "App" section and add a row for a key of
your choosing. If it doesn't exist, add it to the appropriate section.

The pattern for each row is a tuple: `("key", "description")`. Find an existing
row in the popup and copy its structure.

**Verify:**

```bash
cargo build && ./target/debug/playterm
```

Press `i` to open the help popup. Your new entry should appear.

**What you learned:** The popup is built from a list of tuples. Adding a row means
adding one more tuple to the list. Tuples in Rust are written `(value1, value2)`.

---

## Exercise 6: Change the "Not Playing" Message

**Goal:** Change the "Not playing" text in the now-playing bar to "Nothing queued".

**Difficulty:** ⭐⭐ (easy)

**File:** `playterm/src/ui/now_playing.rs`, line 61

```rust
Span::styled("Not playing", Style::default().fg(t.dimmed)),
```

Change it to:

```rust
Span::styled("Nothing queued", Style::default().fg(t.dimmed)),
```

**Verify:**

```bash
cargo build && ./target/debug/playterm
```

Start the app with an empty queue. The now-playing bar should show "Nothing queued".

**What you learned:** UI text in ratatui is wrapped in `Span::styled` with a style.
The `Style::default().fg(color)` part sets the text color. You can also add
`.add_modifier(Modifier::BOLD)` for bold text.

---

## Exercise 7: Adjust the Progress Bar Poll Rate

**Goal:** Change how often the progress bar updates. Currently it refreshes every
50ms; change it to 100ms.

**Difficulty:** ⭐⭐⭐ (medium)

**File:** `playterm/src/main.rs`, line 177

```rust
let poll_ms = if app.accent_transition_active() { 33 } else { 50 };
```

Change `50` to `100`:

```rust
let poll_ms = if app.accent_transition_active() { 33 } else { 100 };
```

**Verify:**

```bash
cargo build && ./target/debug/playterm
```

Play a track and watch the progress bar. It should update more slowly (every 100ms
instead of 50ms). The difference is subtle but visible on the sub-cell fractional
bar blocks.

**What you learned:** The event loop polls for input with a timeout. Shorter timeout
= more responsive UI but slightly more CPU usage. The `Duration::from_millis(n)`
call creates a time duration from a number of milliseconds. The number `100` is a
`u64` here (milliseconds as an integer).

---

## Exercise 8: Add a New Action Variant

**Goal:** Add a new action, `Action::ShowVersion`, triggered by the `v` key. For
now it will just print a message to stderr (eprintln!) since adding a real UI
popup is more work.

**Difficulty:** ⭐⭐⭐ (medium)

This exercise walks you through adding something end-to-end across multiple files.

**Step 1 — Add the variant to the Action enum:**

**File:** `playterm/src/action.rs`

Find the `pub enum Action` block. Add `ShowVersion` before `Quit`:

```rust
ShowVersion,
Quit,
```

**Step 2 — Map the `v` key to the new action:**

**File:** `playterm/src/main.rs`, in the `map_key` function (around line 250)

Add before the `if kb.quit.matches(...)` line:

```rust
if code == KeyCode::Char('v') && modifiers.is_empty() { return Action::ShowVersion; }
```

**Step 3 — Handle the action in dispatch:**

**File:** `playterm/src/app.rs`, in the `dispatch` function

Find the `Action::Quit` case. Add a new case before it:

```rust
Action::ShowVersion => {
    eprintln!("playterm v0.1.0");
}
```

**Step 4 — Handle the new variant in any existing match statements:**

The compiler will tell you if you forgot. Run `cargo build` — if there's a
`non-exhaustive patterns` error, find the match that needs updating and add
`Action::ShowVersion => {}` to it.

**Verify:**

```bash
cargo build 2>&1 | head -30  # check for errors
./target/debug/playterm 2>debug.log  # run, redirect stderr
# press 'v'
# in another terminal: tail -f debug.log  # watch for the version message
```

**What you learned:** Adding a feature to Rust code usually means:
1. Add the new variant/type
2. Map input to it
3. Handle it in the dispatch/match
4. Let the compiler tell you what else needs updating (exhaustive match checking)

---

## Exercise 9: Change the Progress Bar Characters

**Goal:** Replace the fractional Unicode progress bar blocks with simple ASCII
characters (`=` for filled, `-` for empty).

**Difficulty:** ⭐⭐⭐⭐ (medium-hard)

**File:** `playterm/src/ui/now_playing.rs`, lines 161–169

Current code:

```rust
const FRAC: [char; 8] = ['▏', '▎', '▍', '▌', '▋', '▊', '▉', '█'];
let units       = ((ratio * bar_w as f64 * 8.0) as usize).min(bar_w * 8);
let full        = units / 8;
let frac        = units % 8;
let has_partial = frac > 0 && full < bar_w;
let empty       = bar_w - full - usize::from(has_partial);

let filled_str:  String = "█".repeat(full);
let partial_str: String = if has_partial { FRAC[frac - 1].to_string() } else { String::new() };
let empty_str:   String = "░".repeat(empty);
```

To use simple ASCII, replace the logic with:

```rust
let full  = ((ratio * bar_w as f64) as usize).min(bar_w);
let empty = bar_w - full;

let filled_str:  String = "=".repeat(full);
let partial_str: String = String::new();
let empty_str:   String = "-".repeat(empty);
```

**Verify:**

```bash
cargo build && ./target/debug/playterm
```

The progress bar should now show `=====-----` style characters.

To restore the original, use `git checkout playterm/src/ui/now_playing.rs`.

**What you learned:** The `String::repeat(n)` method creates a string with a
character repeated `n` times. The fractional block system uses 8 sub-divisions
per character cell for smooth animation — simplifying it to whole characters loses
that precision but gains simplicity.

---

## Exercise 10: Add a "Total Queue Duration" Display

**Goal:** Show the total duration of all songs in the queue somewhere in the UI.
This is a real feature addition that requires reading the queue data and formatting
a duration.

**Difficulty:** ⭐⭐⭐⭐⭐ (hard)

**Where to display it:** The simplest place is in the status bar, replacing the
empty space between the server URL and the help hint.

**Step 1 — Write a helper function:**

In `playterm/src/state.rs`, add a method to `QueueState`:

```rust
/// Total duration of all songs in the queue.
pub fn total_duration_secs(&self) -> u64 {
    self.songs.iter()
        .filter_map(|s| s.duration)
        .map(|d| d as u64)
        .sum()
}
```

Breaking this down:
- `.iter()` — iterate over songs
- `.filter_map(|s| s.duration)` — take `duration` from each song, skip `None` values
- `.map(|d| d as u64)` — convert `u32` to `u64` to avoid overflow
- `.sum()` — add them all up

**Step 2 — Format the duration:**

In `playterm/src/ui/status_bar.rs`, add a helper function after the `render` function:

```rust
fn format_duration(total_secs: u64) -> String {
    let h = total_secs / 3600;
    let m = (total_secs % 3600) / 60;
    let s = total_secs % 60;
    if h > 0 {
        format!("{}h {}m", h, m)
    } else {
        format!("{}m {:02}s", m, s)
    }
}
```

**Step 3 — Use it in the status bar render:**

In `playterm/src/ui/status_bar.rs`, the `render` function currently builds a `Line`
with the server URL and hint. Add the queue duration between them:

```rust
let queue_secs = app.queue.total_duration_secs();
let queue_dur  = if queue_secs > 0 {
    format!(" · {} in queue", format_duration(queue_secs))
} else {
    String::new()
};
```

Then include `queue_dur` in the `Line::from(vec![...])` after the server URL span.

**Verify:**

```bash
cargo build && ./target/debug/playterm
```

Add some tracks to the queue. The status bar should show something like
`● 192.168.68.122:4533 · 1h 23m in queue` followed by `i — help`.

**What you learned:**
- Iterator chaining with `.filter_map()`, `.map()`, `.sum()`
- Formatting time values with integer division and modulo
- How the status bar is structured
- Adding methods to existing structs with `impl`

---

# Part 5: Where to Go Next

## Essential Resources

### The Rust Book (free online)

`https://doc.rust-lang.org/book/`

The official Rust textbook. Start at Chapter 1 (installation) and work through it
sequentially. Chapters 1–10 will solidify everything in Parts 1 and 2 of this guide.
Don't skip Chapter 4 (ownership) — it's the core of the language.

### Rustlings (interactive exercises)

`https://github.com/rust-lang/rustlings`

Small exercises that make you fix broken code to learn each concept. Install it with:

```bash
cargo install rustlings
rustlings
```

Do this *after* reading at least chapters 1–5 of the Rust Book. The exercises will
feel satisfying rather than frustrating if you have the basics down.

### Exercism — Rust track

`https://exercism.org/tracks/rust`

Coding challenges with mentor feedback. Good for practicing after you've read the
Book. The early exercises are easy; they ramp up quickly.

### docs.rs — library documentation

Every crate used by playterm has documentation at `https://docs.rs/<crate-name>`:
- `https://docs.rs/ratatui` — the TUI framework
- `https://docs.rs/tokio` — async runtime
- `https://docs.rs/serde` — serialization
- `https://docs.rs/anyhow` — error handling

When you're trying to understand what a method does in playterm, `docs.rs` is where
to look.

---

## Feature Ideas for playterm

Here are real features you could add to playterm as you learn. They're ordered from
easier to harder.

### Easy
- **Repeat mode** — add `Action::ToggleRepeat`, a `repeat: bool` field on `QueueState`,
  and modify the `TrackEnded` handler to restart the current track when `repeat` is true
- **Show queue length** — display `{n} tracks · {duration}` somewhere (you did this in
  Exercise 10!)
- **Custom "nothing playing" message** — you did Exercise 6, but add a config option
  so users can set their own message

### Medium
- **Jump to now-playing track** — press a key to jump the queue cursor to the
  currently playing track and scroll it into view
- **Remove from queue** — press `d` to remove the highlighted queue item
- **Seek with percentage** — press `0`–`9` to seek to 0%, 10%, 20%, ..., 90% of
  the current track

### Hard
- **Playlist support** — fetch playlists via the Subsonic `getPlaylists` API endpoint
  and add a new browser column for them
- **Recently played** — display recently played tracks from the `PlayHistory` struct
  (already implemented in `playterm/src/history.rs`) in a new tab
- **Volume indicator** — show the current volume level as a small bar in the status bar,
  updated when `+`/`-` are pressed

---

## How to Read Compiler Errors

This is the most important skill in Rust. The Rust compiler writes excellent error
messages. Here's how to read them:

### The basic structure

```
error[E0502]: cannot borrow `app.cache` as mutable because it is also borrowed as immutable
  --> playterm/src/app.rs:483:9
   |
480|         let path = self.cache.get_const(&song.id);
   |                    -------------------------------- immutable borrow occurs here
483|         self.cache.put(&song.id, ...);
   |         ^^^^^^^^^^ mutable borrow occurs here
```

Every error has:
1. **Error code** (`E0502`) — you can look this up: `https://doc.rust-lang.org/error_codes/E0502.html`
2. **Description** — what went wrong in plain English
3. **File and line** (`playterm/src/app.rs:483:9`) — exactly where the problem is
4. **Annotations** — arrows pointing to the specific code

### The `error[Exxx]` codes

When you see an error like `E0502`, run:

```bash
rustc --explain E0502
```

This gives a detailed explanation with examples of the error and how to fix it.

### The most common errors for beginners

| Error | What it means | Common fix |
|-------|--------------|------------|
| `E0382: use of moved value` | You used a value after transferring ownership | Use `clone()` or borrow with `&` |
| `E0502: cannot borrow as mutable` | Two references conflict | Split the borrows into separate lines |
| `E0308: mismatched types` | Wrong type for an argument | Check the function signature; add conversions |
| `E0507: cannot move out of borrowed content` | Tried to own part of something you borrowed | Clone the value or use a reference |
| `E0277: trait not implemented` | Used a type in a context that requires a trait | Implement the trait or use a type that already implements it |

### The workflow

```bash
cargo build 2>&1 | less   # pipe errors to a pager so they don't scroll away
```

Fix the *first* error first. Often one mistake causes a cascade of downstream errors.
After fixing the first one, run `cargo build` again — many other errors may vanish.

### "Why doesn't the compiler just fix it?"

It often can. Try:

```bash
cargo fix
```

This automatically applies many common fixes suggested by the compiler.

---

## The Mental Model That Makes Everything Click

After working with Rust for a while, you'll internalize a mental model that makes
the borrow checker feel natural instead of adversarial:

**The borrow checker is a proof that your program doesn't have data races.**

Every `&` borrow says "I promise I'm only reading." Every `&mut` says "I'm the only
one changing this." The compiler verifies these promises and refuses to compile code
that would break them.

When the borrow checker rejects your code, it's saying: "If I let this compile,
there would be a situation where two parts of the program modify the same data
simultaneously, and that's a bug." It's not being pedantic — it's catching a real
bug.

The ergonomic response is to design your code so that reads and writes are clearly
separated. playterm does this with channels: the audio thread owns its state, the
TUI thread owns its state, and they communicate through channels. No shared mutable
state means no borrow checker conflicts.

---

## You Already Know More Than You Think

Look at what you now understand:

```rust
// From playterm/src/main.rs, lines 36-42
#[tokio::main]
async fn main() -> Result<()> {
    let config = Config::load().unwrap_or_else(|e| {
        eprintln!("error: {e}");
        process::exit(1);
    });
    let mut app = App::new(config)?;
```

Six months ago you had never read a line of code. Now you can read this and know:
- `#[tokio::main]` sets up the async runtime
- `async fn main()` — the entry point, async so it can use `await`
- `-> Result<()>` — returns success or an error
- `Config::load()` — calls the `load` method on the `Config` type
- `.unwrap_or_else(|e| {...})` — on error, run the closure with the error `e`
- `eprintln!("error: {e}")` — print to stderr with the `{e}` format placeholder
- `process::exit(1)` — exit with code 1 (failure)
- `let mut app = App::new(config)?` — create the App, propagate any error with `?`

That's not "kind of understanding" it. That's *actually* reading Rust code.

The program you've been using for months is no longer a black box. You can open any
file in it, read the code, and understand what it does. That's the beginning of
being a programmer.

---

*This guide was written specifically for the playterm codebase at*
*`~/projects/playterm-app/`. All code examples are real — you can find every*
*snippet in the actual source files at the paths shown.*
