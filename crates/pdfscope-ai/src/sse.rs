//! Server-sent events.
//!
//! Both wire protocols used by PDFScope are SSE, with different framing around the same idea. The
//! parser here is deliberately dumb: it turns bytes into complete frames and never interprets them.
//!
//! The bug this exists to prevent: a frame split across two network reads. The parser buffers
//! incomplete lines and only emits a frame when the blank line arrives.

use futures::{Stream, StreamExt};

use crate::AiError;

/// One dispatched SSE frame.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SseFrame {
    /// The `event:` field, when the protocol uses one.
    pub event: Option<String>,
    /// The `data:` payload, with multiple `data:` lines joined by newlines.
    pub data: String,
}

/// Incremental SSE parser.
#[derive(Debug, Default)]
pub struct SseParser {
    buffer: String,
    event: Option<String>,
    data: String,
}

impl SseParser {
    /// Creates an empty parser.
    pub fn new() -> Self {
        Self::default()
    }

    /// Feeds raw text, returning every frame completed by it.
    pub fn push(&mut self, chunk: &str) -> Vec<SseFrame> {
        self.buffer.push_str(chunk);
        let mut frames = Vec::new();

        while let Some(newline) = self.buffer.find('\n') {
            let line = self.buffer[..newline].trim_end_matches('\r').to_owned();
            self.buffer.drain(..=newline);
            self.line(&line, &mut frames);
        }

        frames
    }

    /// Flushes a final frame that arrived without a trailing blank line.
    pub fn finish(&mut self) -> Option<SseFrame> {
        // A provider may close the connection mid-line; treat what is left as a line.
        if !self.buffer.is_empty() {
            let line = std::mem::take(&mut self.buffer);
            let line = line.trim_end_matches('\r').to_owned();
            let mut ignored = Vec::new();
            self.line(&line, &mut ignored);
        }

        if self.data.is_empty() {
            return None;
        }
        Some(SseFrame {
            event: self.event.take(),
            data: std::mem::take(&mut self.data),
        })
    }

    fn line(&mut self, line: &str, frames: &mut Vec<SseFrame>) {
        if line.is_empty() {
            if !self.data.is_empty() {
                frames.push(SseFrame {
                    event: self.event.take(),
                    data: std::mem::take(&mut self.data),
                });
            } else {
                self.event = None;
            }
            return;
        }

        if line.starts_with(':') {
            // A comment, used by some providers as a keep-alive.
            return;
        }

        if let Some(rest) = line.strip_prefix("event:") {
            self.event = Some(rest.trim().to_owned());
        } else if let Some(rest) = line.strip_prefix("data:") {
            if !self.data.is_empty() {
                self.data.push('\n');
            }
            self.data.push_str(rest.strip_prefix(' ').unwrap_or(rest));
        }
    }
}

/// Turns a streaming HTTP response into frames.
pub fn frames(response: reqwest::Response) -> impl Stream<Item = Result<SseFrame, AiError>> {
    async_stream::try_stream! {
        let body = response.bytes_stream();
        futures::pin_mut!(body);
        let mut parser = SseParser::new();

        while let Some(chunk) = body.next().await {
            let bytes = chunk.map_err(AiError::Transport)?;
            let text = String::from_utf8_lossy(&bytes);
            for frame in parser.push(&text) {
                yield frame;
            }
        }

        if let Some(frame) = parser.finish() {
            yield frame;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_two_frames_from_one_chunk() {
        let mut parser = SseParser::new();
        let frames = parser.push("data: one\n\ndata: two\n\n");
        assert_eq!(frames.len(), 2);
        assert_eq!(frames[0].data, "one");
        assert_eq!(frames[1].data, "two");
    }

    #[test]
    fn buffers_a_frame_split_across_reads() {
        let mut parser = SseParser::new();
        assert!(parser.push("data: {\"a\":").is_empty());
        let frames = parser.push("1}\n\n");
        assert_eq!(frames.len(), 1);
        assert_eq!(frames[0].data, "{\"a\":1}");
    }

    #[test]
    fn handles_crlf_and_comment_lines() {
        let mut parser = SseParser::new();
        let frames = parser.push(": keep-alive\r\ndata: x\r\n\r\n");
        assert_eq!(frames.len(), 1);
        assert_eq!(frames[0].data, "x");
    }

    #[test]
    fn captures_the_event_field() {
        let mut parser = SseParser::new();
        let frames = parser.push("event: message_stop\ndata: {}\n\n");
        assert_eq!(frames[0].event.as_deref(), Some("message_stop"));
    }

    #[test]
    fn flushes_a_trailing_frame_without_a_blank_line() {
        let mut parser = SseParser::new();
        assert!(parser.push("data: last").is_empty());
        assert_eq!(
            parser.finish().map(|frame| frame.data),
            Some("last".to_owned())
        );
    }

    #[test]
    fn joins_multiple_data_lines() {
        let mut parser = SseParser::new();
        let frames = parser.push("data: a\ndata: b\n\n");
        assert_eq!(frames[0].data, "a\nb");
    }
}
