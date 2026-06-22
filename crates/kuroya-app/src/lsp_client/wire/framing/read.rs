use anyhow::{anyhow, bail};
use serde_json::Value;
use std::io;
use tokio::{
    io::{AsyncBufRead, AsyncBufReadExt, AsyncReadExt, BufReader},
    process::ChildStdout,
};

const CONTENT_LENGTH_PREFIX: &[u8] = b"Content-Length:";
const HEADER_LINE_CAPACITY: usize =
    CONTENT_LENGTH_PREFIX.len() + " ".len() + CONTENT_LENGTH_DIGIT_CAPACITY + "\r\n".len();
const CONTENT_LENGTH_DIGIT_CAPACITY: usize = 20;
const MAX_HEADER_BYTES: usize = 64 * 1024;
const MAX_HEADER_LINE_BYTES: usize = 8 * 1024;
const MAX_BODY_BYTES: usize = 16 * 1024 * 1024;
const MAX_BODY_CAPACITY_TO_RETAIN: usize = 4 * 1024 * 1024;

pub(in crate::lsp_client) struct LspMessageReadBuffer {
    header: Vec<u8>,
    body: Vec<u8>,
}

impl Default for LspMessageReadBuffer {
    fn default() -> Self {
        Self {
            header: Vec::with_capacity(HEADER_LINE_CAPACITY),
            body: Vec::new(),
        }
    }
}

pub(in crate::lsp_client) async fn read_message(
    stdout: &mut BufReader<ChildStdout>,
    buffer: &mut LspMessageReadBuffer,
) -> anyhow::Result<Option<Value>> {
    read_message_from(stdout, buffer).await
}

async fn read_message_from<R>(
    reader: &mut R,
    buffer: &mut LspMessageReadBuffer,
) -> anyhow::Result<Option<Value>>
where
    R: AsyncBufRead + Unpin,
{
    let mut content_length = None;
    let mut header_bytes = 0usize;
    let mut saw_header_bytes = false;

    loop {
        buffer.header.clear();
        let bytes = read_header_line(reader, &mut buffer.header).await?;
        if bytes == 0 {
            if saw_header_bytes {
                bail!("server closed stdout before completing LSP headers");
            }
            return Ok(None);
        }
        saw_header_bytes = true;

        if !buffer.header.ends_with(b"\n") {
            bail!("server closed stdout before completing LSP header line");
        }

        header_bytes = header_bytes
            .checked_add(bytes)
            .ok_or_else(|| anyhow!("LSP headers exceed maximum size"))?;
        if header_bytes > MAX_HEADER_BYTES {
            bail!("LSP headers exceed maximum size");
        }

        let trimmed = trim_ascii_end(&buffer.header);
        if trimmed.is_empty() {
            break;
        }

        if let Some(length) = parse_content_length_header(trimmed)? {
            if content_length.is_some() {
                bail!("LSP message contains duplicate Content-Length headers");
            }
            content_length = Some(length);
        }
    }

    let Some(content_length) = content_length else {
        bail!("LSP message is missing Content-Length header");
    };
    prepare_body_buffer(buffer, content_length)?;
    if let Err(error) = reader.read_exact(&mut buffer.body).await {
        release_body_storage(buffer);
        return Err(error.into());
    }
    let value = parse_message_body(buffer)?;
    Ok(Some(value))
}

async fn read_header_line<R>(reader: &mut R, line: &mut Vec<u8>) -> io::Result<usize>
where
    R: AsyncBufRead + Unpin,
{
    let mut bytes_read = 0;

    loop {
        let available = reader.fill_buf().await?;
        if available.is_empty() {
            return Ok(bytes_read);
        }

        let newline_position = available.iter().position(|&byte| byte == b'\n');
        let bytes_to_consume = newline_position.map_or(available.len(), |index| index + 1);
        if bytes_read.saturating_add(bytes_to_consume) > MAX_HEADER_LINE_BYTES {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "LSP header line exceeds maximum length",
            ));
        }

        line.extend_from_slice(&available[..bytes_to_consume]);
        reader.consume(bytes_to_consume);
        bytes_read += bytes_to_consume;

        if newline_position.is_some() {
            return Ok(bytes_read);
        }
    }
}

fn parse_content_length_header(header: &[u8]) -> anyhow::Result<Option<usize>> {
    let Some(length) = header.strip_prefix(CONTENT_LENGTH_PREFIX) else {
        return Ok(None);
    };
    let length = parse_content_length_digits(trim_ascii(length))?;
    if length > MAX_BODY_BYTES {
        bail!("LSP Content-Length {length} exceeds maximum body size {MAX_BODY_BYTES}");
    }
    Ok(Some(length))
}

fn parse_content_length_digits(digits: &[u8]) -> anyhow::Result<usize> {
    if digits.is_empty() {
        bail!("LSP Content-Length header is empty");
    }
    if digits.len() > CONTENT_LENGTH_DIGIT_CAPACITY {
        bail!("LSP Content-Length header has too many digits");
    }

    let mut value = 0usize;
    for &byte in digits {
        if !byte.is_ascii_digit() {
            bail!("LSP Content-Length header contains non-digit bytes");
        }
        let digit = usize::from(byte - b'0');
        value = value
            .checked_mul(10)
            .and_then(|value| value.checked_add(digit))
            .ok_or_else(|| anyhow!("LSP Content-Length overflows usize"))?;
    }

    Ok(value)
}

fn prepare_body_buffer(
    buffer: &mut LspMessageReadBuffer,
    content_length: usize,
) -> anyhow::Result<()> {
    if content_length > MAX_BODY_BYTES {
        bail!("LSP Content-Length {content_length} exceeds maximum body size {MAX_BODY_BYTES}");
    }

    buffer.body.clear();
    let additional = content_length.saturating_sub(buffer.body.capacity());
    buffer
        .body
        .try_reserve_exact(additional)
        .map_err(|error| anyhow!("failed to allocate LSP message body buffer: {error}"))?;
    buffer.body.resize(content_length, 0);
    Ok(())
}

fn parse_message_body(buffer: &mut LspMessageReadBuffer) -> anyhow::Result<Value> {
    let value = serde_json::from_slice(&buffer.body);
    release_body_storage(buffer);
    Ok(value?)
}

fn release_body_storage(buffer: &mut LspMessageReadBuffer) {
    buffer.body.clear();
    if buffer.body.capacity() > MAX_BODY_CAPACITY_TO_RETAIN {
        buffer.body = Vec::new();
    }
}

fn trim_ascii(bytes: &[u8]) -> &[u8] {
    trim_ascii_end(trim_ascii_start(bytes))
}

fn trim_ascii_start(mut bytes: &[u8]) -> &[u8] {
    while let Some(byte) = bytes.first() {
        if !byte.is_ascii_whitespace() {
            break;
        }
        bytes = &bytes[1..];
    }
    bytes
}

fn trim_ascii_end(mut bytes: &[u8]) -> &[u8] {
    while let Some(byte) = bytes.last() {
        if !byte.is_ascii_whitespace() {
            break;
        }
        bytes = &bytes[..bytes.len() - 1];
    }
    bytes
}

#[cfg(test)]
mod tests {
    use super::{
        LspMessageReadBuffer, MAX_BODY_BYTES, MAX_BODY_CAPACITY_TO_RETAIN, MAX_HEADER_BYTES,
        MAX_HEADER_LINE_BYTES, parse_content_length_header, parse_message_body, read_header_line,
        read_message_from, trim_ascii_end,
    };

    #[test]
    fn content_length_header_is_parsed_from_raw_header_bytes() {
        assert_eq!(
            parse_content_length_header(b"Content-Length:\t42  ")
                .expect("content length should parse"),
            Some(42)
        );
        assert_eq!(
            parse_content_length_header(b"Content-Type: application/vscode-jsonrpc; charset=utf-8")
                .expect("unknown headers should be ignored"),
            None
        );
    }

    #[test]
    fn invalid_content_length_header_returns_error() {
        let overflow = format!("Content-Length: {}0", usize::MAX);
        for header in [
            b"Content-Length:" as &[u8],
            b"Content-Length: nope",
            b"Content-Length: -1",
            b"Content-Length: +1",
            b"Content-Length: 1 2",
            b"Content-Length: 1.0",
            overflow.as_bytes(),
        ] {
            assert!(
                parse_content_length_header(header).is_err(),
                "{} should be rejected",
                String::from_utf8_lossy(header)
            );
        }
    }

    #[test]
    fn header_trimming_treats_whitespace_only_line_as_empty() {
        assert_eq!(trim_ascii_end(b" \t\r\n"), b"");
    }

    #[tokio::test]
    async fn read_message_returns_none_on_clean_eof_before_header() {
        let mut reader = tokio::io::BufReader::new(std::io::Cursor::new(Vec::new()));
        let mut buffer = LspMessageReadBuffer::default();

        let message = read_message_from(&mut reader, &mut buffer)
            .await
            .expect("clean eof should not be a framing error");

        assert!(message.is_none());
    }

    #[tokio::test]
    async fn read_message_handles_frame_split_across_small_buffer_reads() {
        let body = br#"{"jsonrpc":"2.0","id":1,"params":{"text":"line\r\nraw\ttext","extra":{"keep":true}}}"#;
        let input = lsp_frame(body);
        let mut reader = tokio::io::BufReader::with_capacity(1, std::io::Cursor::new(input));
        let mut buffer = LspMessageReadBuffer::default();

        let value = read_message_from(&mut reader, &mut buffer)
            .await
            .expect("split frame should parse")
            .expect("frame should produce a message");

        assert_eq!(value["jsonrpc"], "2.0");
        assert_eq!(value["params"]["text"], "line\r\nraw\ttext");
        assert_eq!(value["params"]["extra"]["keep"], true);
    }

    #[tokio::test]
    async fn read_message_preserves_next_frame_after_exact_body_read() {
        let mut input = lsp_frame(br#"{"jsonrpc":"2.0","id":1}"#);
        input.extend_from_slice(&lsp_frame(br#"{"jsonrpc":"2.0","id":2}"#));
        let mut reader = tokio::io::BufReader::with_capacity(7, std::io::Cursor::new(input));
        let mut buffer = LspMessageReadBuffer::default();

        let first = read_message_from(&mut reader, &mut buffer)
            .await
            .expect("first frame should parse")
            .expect("first frame should produce a message");
        let second = read_message_from(&mut reader, &mut buffer)
            .await
            .expect("second frame should parse")
            .expect("second frame should produce a message");

        assert_eq!(first["id"], 1);
        assert_eq!(second["id"], 2);
    }

    #[tokio::test]
    async fn read_message_rejects_partial_header_before_blank_line() {
        let mut reader =
            tokio::io::BufReader::new(std::io::Cursor::new(b"Content-Length: 2\r\n{}".to_vec()));
        let mut buffer = LspMessageReadBuffer::default();

        let error = read_message_from(&mut reader, &mut buffer)
            .await
            .expect_err("partial header block should be rejected");

        assert!(error.to_string().contains("LSP header line"));
        assert_eq!(buffer.body.len(), 0);
    }

    #[tokio::test]
    async fn read_message_rejects_partial_body_and_clears_buffer() {
        let mut reader = tokio::io::BufReader::new(std::io::Cursor::new(
            b"Content-Length: 4\r\n\r\n{}".to_vec(),
        ));
        let mut buffer = LspMessageReadBuffer::default();

        let error = read_message_from(&mut reader, &mut buffer)
            .await
            .expect_err("truncated body should be rejected");

        let io_error = error
            .downcast_ref::<std::io::Error>()
            .expect("truncated body should be reported as an io error");
        assert_eq!(io_error.kind(), std::io::ErrorKind::UnexpectedEof);
        assert_eq!(buffer.body.len(), 0);
    }

    #[tokio::test]
    async fn read_message_rejects_missing_content_length_header() {
        let mut reader = tokio::io::BufReader::new(std::io::Cursor::new(
            b"Content-Type: application/vscode-jsonrpc; charset=utf-8\r\n\r\n{}".to_vec(),
        ));
        let mut buffer = LspMessageReadBuffer::default();

        let error = read_message_from(&mut reader, &mut buffer)
            .await
            .expect_err("header block without Content-Length should be rejected");

        assert!(error.to_string().contains("missing Content-Length"));
        assert_eq!(buffer.body.len(), 0);
    }

    #[tokio::test]
    async fn read_message_rejects_duplicate_content_length_headers() {
        let mut reader = tokio::io::BufReader::new(std::io::Cursor::new(
            b"Content-Length: 2\r\nContent-Length: 2\r\n\r\n{}".to_vec(),
        ));
        let mut buffer = LspMessageReadBuffer::default();

        let error = read_message_from(&mut reader, &mut buffer)
            .await
            .expect_err("duplicate Content-Length should be rejected");

        assert!(error.to_string().contains("duplicate Content-Length"));
        assert_eq!(buffer.body.len(), 0);
    }

    #[tokio::test]
    async fn read_message_rejects_oversized_content_length_before_body_allocation() {
        let input = format!("Content-Length: {}\r\n\r\n", MAX_BODY_BYTES + 1).into_bytes();
        let mut reader = tokio::io::BufReader::new(std::io::Cursor::new(input));
        let mut buffer = LspMessageReadBuffer::default();
        let body_capacity = buffer.body.capacity();

        let error = read_message_from(&mut reader, &mut buffer)
            .await
            .expect_err("oversized body should be rejected before read");

        assert!(error.to_string().contains("maximum body size"));
        assert_eq!(buffer.body.len(), 0);
        assert_eq!(buffer.body.capacity(), body_capacity);
    }

    #[tokio::test]
    async fn read_message_rejects_total_header_bytes_over_budget() {
        let mut input = Vec::new();
        while input.len() <= MAX_HEADER_BYTES {
            input.extend_from_slice(b"X-Test: value\r\n");
        }
        input.extend_from_slice(b"\r\n");
        let mut reader = tokio::io::BufReader::new(std::io::Cursor::new(input));
        let mut buffer = LspMessageReadBuffer::default();

        let error = read_message_from(&mut reader, &mut buffer)
            .await
            .expect_err("oversized header block should be rejected");

        assert!(error.to_string().contains("headers exceed maximum size"));
        assert_eq!(buffer.body.len(), 0);
    }

    #[tokio::test]
    async fn read_header_line_rejects_overlong_line_before_newline() {
        let input = vec![b'x'; MAX_HEADER_LINE_BYTES + 1];
        let mut reader = tokio::io::BufReader::new(std::io::Cursor::new(input));
        let mut line = Vec::new();

        let error = read_header_line(&mut reader, &mut line)
            .await
            .expect_err("overlong header line should fail");

        assert_eq!(error.kind(), std::io::ErrorKind::InvalidData);
        assert!(line.len() <= MAX_HEADER_LINE_BYTES);
    }

    #[test]
    fn parsed_body_releases_oversized_retained_allocation() {
        let mut buffer = LspMessageReadBuffer::default();
        buffer
            .body
            .reserve_exact(MAX_BODY_CAPACITY_TO_RETAIN + 1024);
        buffer.body.extend_from_slice(br#"{"jsonrpc":"2.0"}"#);

        let value = parse_message_body(&mut buffer).expect("body should parse");

        assert_eq!(value["jsonrpc"], "2.0");
        assert_eq!(buffer.body.len(), 0);
        assert_eq!(buffer.body.capacity(), 0);
    }

    #[test]
    fn invalid_body_is_cleared_before_error_returns() {
        let mut buffer = LspMessageReadBuffer::default();
        buffer
            .body
            .reserve_exact(MAX_BODY_CAPACITY_TO_RETAIN + 1024);
        buffer.body.extend_from_slice(b"{invalid");

        assert!(parse_message_body(&mut buffer).is_err());
        assert_eq!(buffer.body.len(), 0);
        assert_eq!(buffer.body.capacity(), 0);
    }

    fn lsp_frame(body: &[u8]) -> Vec<u8> {
        let mut frame = format!("Content-Length: {}\r\n\r\n", body.len()).into_bytes();
        frame.extend_from_slice(body);
        frame
    }
}
