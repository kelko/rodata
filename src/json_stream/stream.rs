//! Utilities to synchronously decode Json.
//!
//! For use in scenarios where it's okay to block waiting for more input while decoding.
use crate::json_stream::decode::{ConsumableBytes, DecodeError, JsonDecoder};
use crate::json_stream::token::{JsonString, JsonToken};
use std::io::{Error as IoError};
use std::str;

use bytes::Bytes;
use futures::StreamExt;
use async_trait::async_trait;

#[derive(Debug)]
/// Error type from synchronous TokenIterator s.
pub enum Error {
    /// Propagated error from the underlying Read object
    IoError(IoError),
    /// Error that occurred during Json decoding.
    DecodeError(DecodeError),
}

impl From<IoError> for Error {
    fn from(io: IoError) -> Error {
        Error::IoError(io)
    }
}

impl From<DecodeError> for Error {
    fn from(d: DecodeError) -> Error {
        Error::DecodeError(d)
    }
}

#[async_trait]
/// Trait for an iterator over JsonTokens.
pub trait TokenIterator {
    /// Advance to the next token.
    async fn advance(&mut self) -> Result<(), Error>;

    /// Get the current token, or None if the stream is exhausted.
    fn get(&self) -> Option<JsonToken<'_>>;

    /// Advance to the next token, then get the current token.
    ///
    ///
    /// Implemented as a call to `advance()` and then `get()`
    async fn next(&mut self) -> Result<Option<JsonToken<'_>>, Error> {
        self.advance().await?;
        Ok(self.get())
    }
}

#[derive(Debug)]
struct StreamIndices {
    start: usize,
    scanned: usize,
    end: usize,
}

struct Slice {
    start: usize,
    end: usize,
}

impl Slice {
    fn new(haystack: &[u8], needle: &str) -> Slice {
        let start = (needle.as_ptr() as usize) - (haystack.as_ptr() as usize);
        let end = start + needle.len();
        Slice { start, end }
    }

    fn to_str<'a>(&self, buffer: &'a [u8]) -> &'a str {
        unsafe { str::from_utf8_unchecked(&buffer[self.start..self.end]) }
    }

    fn to_json_string<'a>(&self, buffer: &'a [u8]) -> JsonString<'a> {
        unsafe { JsonString::from_str_unchecked(self.to_str(buffer)) }
    }
}

enum DerefJsonToken {
    StartObject,
    EndObject,
    StartArray,
    EndArray,
    JsNull,
    JsBoolean(bool),
    JsNumber(Slice),
    JsString(Slice),
    JsKey(Slice),
}

impl DerefJsonToken {
    fn new<'a>(token: JsonToken<'a>, buffer: &'a [u8]) -> DerefJsonToken {
        match token {
            JsonToken::StartObject => DerefJsonToken::StartObject,
            JsonToken::EndObject => DerefJsonToken::EndObject,
            JsonToken::StartArray => DerefJsonToken::StartArray,
            JsonToken::EndArray => DerefJsonToken::EndArray,
            JsonToken::JsNull => DerefJsonToken::JsNull,
            JsonToken::JsBoolean(b) => DerefJsonToken::JsBoolean(b),
            JsonToken::JsNumber(s) => DerefJsonToken::JsNumber(Slice::new(buffer, s)),
            JsonToken::JsString(s) => {
                DerefJsonToken::JsString(Slice::new(buffer, s.into_raw_str()))
            }
            JsonToken::JsKey(s) => DerefJsonToken::JsKey(Slice::new(buffer, s.into_raw_str())),
        }
    }

    // unsafe because caller is responsible for ensuring buffer contents haven't changed since
    // construction.
    unsafe fn reref<'a>(&self, buffer: &'a [u8]) -> JsonToken<'a> {
        match self {
            DerefJsonToken::StartObject => JsonToken::StartObject,
            DerefJsonToken::EndObject => JsonToken::EndObject,
            DerefJsonToken::StartArray => JsonToken::StartArray,
            DerefJsonToken::EndArray => JsonToken::EndArray,
            DerefJsonToken::JsNull => JsonToken::JsNull,
            DerefJsonToken::JsBoolean(b) => JsonToken::JsBoolean(*b),
            DerefJsonToken::JsNumber(s) => JsonToken::JsNumber(s.to_str(buffer)),
            DerefJsonToken::JsString(s) => JsonToken::JsString(s.to_json_string(buffer)),
            DerefJsonToken::JsKey(s) => JsonToken::JsKey(s.to_json_string(buffer)),
        }
    }
}

/// Stream of JSON values from an std::io::Read built on top of a decoder.
pub struct Stream<R> {
    buffer: Vec<u8>,
    indices: StreamIndices,
    decoder: JsonDecoder,
    curr_token: Option<DerefJsonToken>,
    seen_eof: bool,
    src: R,
}

// RAII structure for creating consumable bytes that advance the underlying buffer.
struct ConsumeableByteAdvance<'a, 'b> {
    source: &'b mut StreamIndices,
    bytes: ConsumableBytes<'a>,
    in_len: usize,
}

impl<'a, 'b> Drop for ConsumeableByteAdvance<'a, 'b> {
    fn drop(&mut self) {
        self.source.start += self.in_len - self.bytes.len();
        if self.source.scanned < self.source.start {
            self.source.scanned = self.source.start;
        }
    }
}

impl<'a, 'b> std::fmt::Display for ConsumeableByteAdvance<'a, 'b> {    
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::result::Result<(), std::fmt::Error> { 
        formatter.write_fmt(format_args!("{}", self.bytes))
     }
}

impl<'a, 'b> ConsumeableByteAdvance<'a, 'b> {
    fn new(
        source: &'b mut StreamIndices,
        end: bool,
        bytes: &'a [u8],
    ) -> ConsumeableByteAdvance<'a, 'b> {
        let in_len = source.end - source.start;
        let bytes = if end {
            ConsumableBytes::new_end_of_stream(&bytes[source.start..source.end])
        } else {
            ConsumableBytes::new(&bytes[source.start..source.end])
        };
        ConsumeableByteAdvance {
            source,
            bytes,
            in_len,
        }
    }

    fn scanned_to_end(&mut self) {
        self.source.scanned = self.source.end;
    }

    fn bytes(&mut self) -> &mut ConsumableBytes<'a> {
        &mut self.bytes
    }
}

#[async_trait]
impl<R> TokenIterator for Stream<R>
where
    R: futures::stream::Stream<Item = reqwest::Result<Bytes>> + Unpin + Send,
{
    /// Advance to the next token.
    async fn advance(&mut self) -> Result<(), Error> {
        self.curr_token = self.advance_impl().await?;
        Ok(())
    }

    /// Get the current token, or None if the stream is exhausted.
    fn get(&self) -> Option<JsonToken<'_>> {
        let b = &self.buffer[..];
        // reref is okay because every time buffer is changed, curr_token is updated.
        //
        // Equivalently, the contents of self.buffer won't have changed since the previous creation
        // of curr_token.
        //
        // This lets us re-interpret bytes as a str without re-checking UTF encoding every time.
        self.curr_token.as_ref().map(|d| unsafe { d.reref(b) })
    }
}

impl<R> Stream<R>
where
    R: futures::stream::Stream<Item = reqwest::Result<Bytes>> + Unpin + Send,
{
    const BUFFER_SIZE: usize = 1_048_576;

    /// Create a Stream from a std::io::Read.
    pub fn from_stream(src: R) -> Result<Stream<R>, Error> {
        Self::from_stream_with_initial_capacity(src, Stream::<R>::BUFFER_SIZE)
    }

    /// Create a Stream from a std::io::Read, with specified initial capacity.
    pub fn from_stream_with_initial_capacity(src: R, cap: usize) -> Result<Stream<R>, Error> {
        Ok(Stream {
            buffer: vec![0; cap],
            indices: StreamIndices {
                start: 0,
                scanned: 0,
                end: 0,
            },
            decoder: JsonDecoder::new(),
            curr_token: None,
            seen_eof: false,
            src,
        })
    }

    async fn ensure_bytes(&mut self) -> Result<(), IoError> {
        
        if self.indices.scanned >= self.indices.end {
            
            if self.indices.start > 0 {
                let to_move = self.indices.end - self.indices.start;
                if to_move > 0 {
                    // TODO: use copy_within once stable.
                    let mut new_buff = vec![0; self.buffer.len()];
                    new_buff[..to_move]
                        .copy_from_slice(&self.buffer[self.indices.start..self.indices.end]);
                    self.buffer = new_buff;
                }
                self.indices.end = to_move;
                self.indices.scanned -= self.indices.start;
                self.indices.start = 0;
            }

            let target_bytes = &mut self.buffer[self.indices.scanned..];
            assert!(
                !target_bytes.is_empty(),
                "Need to add shuffling / compacting: {:?}, {:?}",
                self.indices,
                self.buffer.len()
            );

            if let Some(Ok(chunk_bytes)) = self.src.next().await {
                let chunk_length = chunk_bytes.len();
                let calculated_end = self.indices.scanned + chunk_length;

                while calculated_end > self.buffer.len() {
                    self.buffer.resize(self.buffer.len() * 2, b'0');
                }

                self.buffer[self.indices.scanned..calculated_end].copy_from_slice(&chunk_bytes);
                self.indices.end = self.indices.scanned + chunk_length;

            } else {
                self.seen_eof = true;
            }
        }
        Ok(())
    }

    fn decode<'a>(
        buffer: &'a [u8],
        eof: bool,
        indices: &mut StreamIndices,
        decoder: &mut JsonDecoder,
    ) -> Result<Option<JsonToken<'a>>, Error> {
        let mut cb = ConsumeableByteAdvance::new(indices, eof, buffer);
        let r = decoder.decode(cb.bytes());
        if let Err(DecodeError::NeedsMore) = r {
            cb.scanned_to_end()
        }
        Ok(r?)
    }

    async fn advance_impl(&mut self) -> Result<Option<DerefJsonToken>, Error> {
        loop {
            self.ensure_bytes().await?;
            match Self::decode(
                &self.buffer[..],
                self.seen_eof,
                &mut self.indices,
                &mut self.decoder,
            ) {
                Err(Error::DecodeError(DecodeError::NeedsMore)) => {
                    assert!(!self.seen_eof, "Cannot return NeedsMore if we've seen eof.");
                }
                n => {
                    return n.map(|o| o.map(|t| DerefJsonToken::new(t, &self.buffer[..])));
                }
            }
        }
    }
}
