pub mod client;
pub mod error;
pub mod models;

pub use client::{
    fetch_library, fetch_songs_for_artist, SubsonicClient, DEFAULT_SERVER_URL,
};
pub use error::SubsonicError;
pub use models::{Album, Artist, ArtistIndex, Artists, SearchResult3, Song, SubsonicLibrary};
