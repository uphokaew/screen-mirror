/// Audio decoding and playback module
pub mod decoder;
pub mod player;

pub use decoder::{DecodedAudio, HardwareAudioDecoder};
pub use player::AudioPlayer;
