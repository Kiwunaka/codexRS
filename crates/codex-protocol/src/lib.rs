use std::error::Error;
use std::fmt;
use std::mem;
use std::num::NonZeroUsize;

pub const DEFAULT_MAX_FRAME_BYTES: usize = 16 * 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FrameTooLarge {
    pub limit: usize,
    pub observed_at_least: usize,
}

impl fmt::Display for FrameTooLarge {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "protocol frame exceeded {} bytes (observed at least {})",
            self.limit, self.observed_at_least
        )
    }
}

impl Error for FrameTooLarge {}

/// Incrementally decodes newline-delimited frames without an unbounded line
/// allocation.
#[derive(Debug)]
pub struct BoundedLineDecoder {
    max_frame_bytes: NonZeroUsize,
    buffer: Vec<u8>,
}

impl BoundedLineDecoder {
    #[must_use]
    pub fn new(max_frame_bytes: NonZeroUsize) -> Self {
        Self {
            max_frame_bytes,
            buffer: Vec::new(),
        }
    }

    pub fn feed(&mut self, chunk: &[u8]) -> Result<Vec<Vec<u8>>, FrameTooLarge> {
        let mut frames = Vec::new();

        for &byte in chunk {
            if byte == b'\n' {
                if self.buffer.last() == Some(&b'\r') {
                    self.buffer.pop();
                }
                frames.push(mem::take(&mut self.buffer));
                continue;
            }

            if self.buffer.len() == self.max_frame_bytes.get() {
                self.buffer.clear();
                return Err(FrameTooLarge {
                    limit: self.max_frame_bytes.get(),
                    observed_at_least: self.max_frame_bytes.get() + 1,
                });
            }

            self.buffer.push(byte);
        }

        Ok(frames)
    }

    #[must_use]
    pub fn finish(&mut self) -> Option<Vec<u8>> {
        if self.buffer.is_empty() {
            None
        } else {
            Some(mem::take(&mut self.buffer))
        }
    }
}

#[cfg(test)]
mod tests {
    use std::num::NonZeroUsize;

    use super::BoundedLineDecoder;

    #[test]
    fn decodes_fragmented_and_crlf_frames() {
        let limit = NonZeroUsize::new(32).unwrap_or(NonZeroUsize::MIN);
        let mut decoder = BoundedLineDecoder::new(limit);

        let first = decoder.feed(b"{\"id\":");
        assert!(matches!(first, Ok(frames) if frames.is_empty()));
        let frames = match decoder.feed(b"1}\r\n{\"id\":2}\n") {
            Ok(frames) => frames,
            Err(error) => panic!("unexpected frame error: {error}"),
        };

        assert_eq!(frames, [br#"{"id":1}"#.to_vec(), br#"{"id":2}"#.to_vec()]);
    }

    #[test]
    fn rejects_oversized_frame_before_growing_further() {
        let limit = NonZeroUsize::new(4).unwrap_or(NonZeroUsize::MIN);
        let mut decoder = BoundedLineDecoder::new(limit);

        let error = match decoder.feed(b"12345") {
            Err(error) => error,
            Ok(frames) => panic!("expected an oversized-frame error, got {frames:?}"),
        };

        assert_eq!(error.limit, 4);
        assert_eq!(error.observed_at_least, 5);
        assert!(decoder.finish().is_none());
    }

    #[test]
    fn returns_final_unterminated_frame() {
        let limit = NonZeroUsize::new(16).unwrap_or(NonZeroUsize::MIN);
        let mut decoder = BoundedLineDecoder::new(limit);
        let frames = decoder.feed(b"last frame");
        assert!(matches!(frames, Ok(frames) if frames.is_empty()));
        assert_eq!(decoder.finish(), Some(b"last frame".to_vec()));
    }
}
