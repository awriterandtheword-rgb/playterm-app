//! Subsonic REST API client.
//!
//! Implements the [Subsonic API](http://www.subsonic.org/pages/api.jsp) v1.16.1
//! with MD5 token authentication against a Navidrome server.
//!
//! # Authentication
//!
//! Every request appends five standard query parameters:
//!
//! | Param | Value |
//! |-------|-------|
//! | `u`   | username |
//! | `t`   | MD5(password + salt) as lowercase hex |
//! | `s`   | random alphanumeric salt |
//! | `v`   | Subsonic API version (`1.16.1`) |
//! | `c`   | client name (`playterm`) |

use anyhow::{Result, anyhow};
use reqwest::{Client, ClientBuilder};
use std::time::Duration;

use crate::error::check_status;
use crate::models::{
    Album, AlbumEnvelope, Artist, ArtistEnvelope, Artists, ArtistsEnvelope, PingEnvelope,
    SearchEnvelope, SearchResult3, Song, SongEnvelope, SubsonicLibrary,
};

// ── Constants ──────────────────────────────────────────────────────────────────

/// Default Navidrome server used when no URL is supplied.
pub const DEFAULT_SERVER_URL: &str = "http://192.168.68.122:4533";

const API_VERSION: &str = "1.16.1";
const CLIENT_NAME: &str = "playterm";

// ── Auth helpers ───────────────────────────────────────────────────────────────

/// Derive a Subsonic token: MD5(password + salt) rendered as lowercase hex.
fn make_token(password: &str, salt: &str) -> String {
    hex::encode(md5::compute(format!("{password}{salt}")).as_ref())
}

/// Generate `len` random lowercase alphanumeric characters for use as a salt.
///
/// Uses a simple LCG seeded from the current system time — sufficient
/// entropy for a per-request Subsonic salt.
fn random_ascii(len: usize) -> String {
    use std::time::SystemTime;
    let charset = b"abcdefghijklmnopqrstuvwxyz0123456789";
    let seed = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0xdead_beef_cafe_babe);
    let mut x = seed;
    (0..len)
        .map(|i| {
            // Knuth multiplicative hash step
            x = x
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407 + i as u64);
            charset[(x >> 33) as usize % charset.len()] as char
        })
        .collect()
}

// ── Client ─────────────────────────────────────────────────────────────────────

/// Async Subsonic API client.
///
/// Create one instance and reuse it — the underlying `reqwest::Client` maintains
/// a connection pool.
///
/// ```no_run
/// # use playterm_subsonic::client::{SubsonicClient, DEFAULT_SERVER_URL};
/// # async fn example() -> anyhow::Result<()> {
/// let client = SubsonicClient::new(DEFAULT_SERVER_URL, "admin", "s3cr3t")?;
/// client.ping().await?;
/// # Ok(())
/// # }
/// ```
#[derive(Clone)]
pub struct SubsonicClient {
    base_url: String,
    username: String,
    password: String,
    http: Client,
}

impl SubsonicClient {
    /// Create a new client.
    ///
    /// `base_url` should be the server root, e.g. `"http://192.168.68.122:4533"`.
    /// Trailing slashes are stripped automatically.
    pub fn new(base_url: &str, username: &str, password: &str) -> Result<Self> {
        let http = ClientBuilder::new()
            .timeout(Duration::from_secs(30))
            .build()?;
        Ok(Self {
            base_url: base_url.trim_end_matches('/').to_string(),
            username: username.to_string(),
            password: password.to_string(),
            http,
        })
    }

    // ── Private helpers ────────────────────────────────────────────────────────

    /// Build the standard authentication parameters.
    ///
    /// A fresh random salt — and therefore a fresh token — is generated on
    /// every call so that repeated requests are not replayable.
    fn auth_params(&self) -> Vec<(&'static str, String)> {
        let salt = random_ascii(12);
        let token = make_token(&self.password, &salt);
        vec![
            ("u", self.username.clone()),
            ("t", token),
            ("s", salt),
            ("v", API_VERSION.to_string()),
            ("c", CLIENT_NAME.to_string()),
            ("f", "json".to_string()),
        ]
    }

    fn endpoint_url(&self, name: &str) -> String {
        format!("{}/rest/{name}", self.base_url)
    }

    // ── Public API ─────────────────────────────────────────────────────────────

    /// Ping the server to verify connectivity and authentication.
    pub async fn ping(&self) -> Result<()> {
        let env: PingEnvelope = self
            .http
            .get(self.endpoint_url("ping"))
            .query(&self.auth_params())
            .send()
            .await?
            .json()
            .await?;
        check_status(&env.response.status, env.response.error.as_ref())
    }

    /// Fetch all artists, grouped by index letter (`getArtists`).
    pub async fn get_artists(&self) -> Result<Artists> {
        let env: ArtistsEnvelope = self
            .http
            .get(self.endpoint_url("getArtists"))
            .query(&self.auth_params())
            .send()
            .await?
            .json()
            .await?;
        let r = &env.response;
        check_status(&r.status, r.error.as_ref())?;
        r.artists
            .clone()
            .ok_or_else(|| anyhow!("missing 'artists' field in getArtists response"))
    }

    /// Fetch a single artist by ID, including album stubs (`getArtist`).
    pub async fn get_artist(&self, id: &str) -> Result<Artist> {
        let mut params = self.auth_params();
        params.push(("id", id.to_string()));
        let env: ArtistEnvelope = self
            .http
            .get(self.endpoint_url("getArtist"))
            .query(&params)
            .send()
            .await?
            .json()
            .await?;
        let r = &env.response;
        check_status(&r.status, r.error.as_ref())?;
        r.artist
            .clone()
            .ok_or_else(|| anyhow!("missing 'artist' field in getArtist response"))
    }

    /// Fetch a full album including its track list by album ID (`getAlbum`).
    pub async fn get_album(&self, id: &str) -> Result<Album> {
        let mut params = self.auth_params();
        params.push(("id", id.to_string()));
        let env: AlbumEnvelope = self
            .http
            .get(self.endpoint_url("getAlbum"))
            .query(&params)
            .send()
            .await?
            .json()
            .await?;
        let r = &env.response;
        check_status(&r.status, r.error.as_ref())?;
        r.album
            .clone()
            .ok_or_else(|| anyhow!("missing 'album' field in getAlbum response"))
    }

    /// Fetch a single song by its ID (`getSong`).
    pub async fn get_song(&self, id: &str) -> Result<Song> {
        let mut params = self.auth_params();
        params.push(("id", id.to_string()));
        let env: SongEnvelope = self
            .http
            .get(self.endpoint_url("getSong"))
            .query(&params)
            .send()
            .await?
            .json()
            .await?;
        let r = &env.response;
        check_status(&r.status, r.error.as_ref())?;
        r.song
            .clone()
            .ok_or_else(|| anyhow!("missing 'song' field in getSong response"))
    }

    /// Construct a signed streaming URL for a song (`stream`).
    ///
    /// The returned URL is self-contained and can be handed directly to a media
    /// player without any further signing.
    ///
    /// Set `max_bit_rate` to `0` to request the original file without
    /// transcoding.
    #[must_use]
    pub fn stream_url(&self, id: &str, max_bit_rate: u32) -> String {
        let params = self.auth_params();
        let mut parts: Vec<String> = params
            .iter()
            .map(|(k, v)| format!("{k}={v}"))
            .collect();
        parts.push(format!("id={id}"));
        parts.push(format!("maxBitRate={max_bit_rate}"));
        format!("{}/rest/stream?{}", self.base_url, parts.join("&"))
    }

    /// Search for artists, albums, and songs matching `query` (`search3`).
    pub async fn search3(
        &self,
        query: &str,
        artist_count: u32,
        album_count: u32,
        song_count: u32,
    ) -> Result<SearchResult3> {
        let mut params = self.auth_params();
        params.push(("query", query.to_string()));
        params.push(("artistCount", artist_count.to_string()));
        params.push(("albumCount", album_count.to_string()));
        params.push(("songCount", song_count.to_string()));
        let env: SearchEnvelope = self
            .http
            .get(self.endpoint_url("search3"))
            .query(&params)
            .send()
            .await?
            .json()
            .await?;
        let r = &env.response;
        check_status(&r.status, r.error.as_ref())?;
        r.search_result3
            .clone()
            .ok_or_else(|| anyhow!("missing 'searchResult3' field in search3 response"))
    }

    /// Mark a song as played (scrobble).
    pub async fn scrobble(&self, id: &str) -> Result<()> {
        let mut params = self.auth_params();
        params.push(("id", id.to_string()));
        params.push(("submission", "true".to_string()));
        let env: PingEnvelope = self
            .http
            .get(self.endpoint_url("scrobble"))
            .query(&params)
            .send()
            .await?
            .json()
            .await?;
        check_status(&env.response.status, env.response.error.as_ref())
    }
}

// ── Library helpers ────────────────────────────────────────────────────────────

/// Fetch the top-level artist list. One network request.
pub async fn fetch_library(client: &SubsonicClient) -> Result<SubsonicLibrary> {
    let artists_response = client.get_artists().await?;
    let mut artists: Vec<Artist> = artists_response
        .index
        .into_iter()
        .flat_map(|bucket| bucket.artist)
        .collect();
    artists.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(SubsonicLibrary { artists })
}

/// Fetch all songs for a single artist: calls `getArtist` for album stubs, then
/// `getAlbum` for each album sequentially.
///
/// Returns a flat, disc+track-number-sorted `Vec<Song>` across all albums.
pub async fn fetch_songs_for_artist(client: &SubsonicClient, artist: &Artist) -> Vec<Song> {
    let artist_detail = match client.get_artist(&artist.id).await {
        Ok(a) => a,
        Err(e) => {
            eprintln!("playterm-subsonic: get_artist({}) failed — {e}", artist.id);
            return Vec::new();
        }
    };

    let mut songs: Vec<Song> = Vec::new();
    for album_stub in &artist_detail.album {
        match client.get_album(&album_stub.id).await {
            Ok(album) => songs.extend(album.song),
            Err(e) => eprintln!("playterm-subsonic: get_album({}) failed — {e}", album_stub.id),
        }
    }

    songs.sort_by_key(|s| (s.disc_number.unwrap_or(1), s.track.unwrap_or(0)));
    songs
}

// ── Tests ──────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    /// Build a test client from environment variables, falling back to the
    /// hard-coded Navidrome instance.
    ///
    /// Override at runtime:
    /// ```sh
    /// SUBSONIC_URL=http://... SUBSONIC_USER=alice SUBSONIC_PASS=s3cr3t \
    ///   cargo test -p playterm-subsonic -- --nocapture
    /// ```
    fn test_client() -> SubsonicClient {
        let url = std::env::var("SUBSONIC_URL")
            .or_else(|_| std::env::var("TERMUSIC_SUBSONIC_URL"))
            .unwrap_or_else(|_| DEFAULT_SERVER_URL.to_string());
        let user = std::env::var("SUBSONIC_USER")
            .or_else(|_| std::env::var("TERMUSIC_SUBSONIC_USER"))
            .unwrap_or_else(|_| "admin".to_string());
        let pass = std::env::var("SUBSONIC_PASS")
            .or_else(|_| std::env::var("TERMUSIC_SUBSONIC_PASS"))
            .unwrap_or_else(|_| "REDACTED".to_string());
        SubsonicClient::new(&url, &user, &pass).expect("client construction must not fail")
    }

    /// Live integration test — pings the Navidrome instance to verify that
    /// MD5 token auth is wired up correctly.
    #[tokio::test]
    async fn ping_live_navidrome() {
        let client = test_client();
        client
            .ping()
            .await
            .expect("ping must succeed against live Navidrome — check credentials and connectivity");
        println!("ping OK");
    }
}
