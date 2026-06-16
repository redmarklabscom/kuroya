use serde_json::Value;
use std::io::{self, IoSlice};
use tokio::{
    io::{AsyncWrite, AsyncWriteExt},
    process::ChildStdin,
};

const CONTENT_LENGTH_PREFIX: &[u8] = b"Content-Length: ";
const HEADER_SUFFIX: &[u8] = b"\r\n\r\n";
const CONTENT_LENGTH_HEADER_CAPACITY: usize =
    CONTENT_LENGTH_PREFIX.len() + CONTENT_LENGTH_DIGIT_CAPACITY + HEADER_SUFFIX.len();
const CONTENT_LENGTH_DIGIT_CAPACITY: usize = 20;

pub(in crate::lsp_client) async fn write_message(
    stdin: &mut ChildStdin,
    value: &Value,
) -> anyhow::Result<()> {
    let body = serde_json::to_vec(value)?;
    let (header, header_len) = content_length_header(body.len());
    write_frame(stdin, &header[..header_len], &body).await?;
    stdin.flush().await?;
    Ok(())
}

async fn write_frame<W>(writer: &mut W, header: &[u8], body: &[u8]) -> io::Result<()>
where
    W: AsyncWrite + Unpin + ?Sized,
{
    if !writer.is_write_vectored() {
        writer.write_all(header).await?;
        writer.write_all(body).await?;
        return Ok(());
    }

    let mut header_written = 0;
    let mut body_written = 0;

    while header_written < header.len() || body_written < body.len() {
        let buffers = [
            IoSlice::new(&header[header_written..]),
            IoSlice::new(&body[body_written..]),
        ];
        let buffers = if header_written == header.len() {
            &buffers[1..]
        } else {
            &buffers[..]
        };

        let written = writer.write_vectored(buffers).await?;
        if written == 0 {
            return Err(io::Error::new(
                io::ErrorKind::WriteZero,
                "failed to write LSP frame",
            ));
        }

        advance_frame_offsets(
            written,
            header,
            body,
            &mut header_written,
            &mut body_written,
        )?;
    }

    Ok(())
}

fn advance_frame_offsets(
    mut written: usize,
    header: &[u8],
    body: &[u8],
    header_written: &mut usize,
    body_written: &mut usize,
) -> io::Result<()> {
    let header_advance = if *header_written < header.len() {
        let header_remaining = header.len() - *header_written;
        let header_advance = header_remaining.min(written);
        written -= header_advance;
        header_advance
    } else {
        0
    };

    let body_remaining = body.len() - *body_written;
    if written > body_remaining {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "writer reported more bytes than the LSP frame contains",
        ));
    }

    *header_written += header_advance;
    *body_written += written;

    Ok(())
}

fn content_length_header(content_length: usize) -> ([u8; CONTENT_LENGTH_HEADER_CAPACITY], usize) {
    let mut header = [0; CONTENT_LENGTH_HEADER_CAPACITY];
    let mut len = 0;

    header[..CONTENT_LENGTH_PREFIX.len()].copy_from_slice(CONTENT_LENGTH_PREFIX);
    len += CONTENT_LENGTH_PREFIX.len();

    let mut digits = [0; CONTENT_LENGTH_DIGIT_CAPACITY];
    let mut digit_count = 0;
    let mut remaining = content_length;
    loop {
        digits[digit_count] = b'0' + (remaining % 10) as u8;
        digit_count += 1;
        remaining /= 10;
        if remaining == 0 {
            break;
        }
    }

    for digit in digits[..digit_count].iter().rev() {
        header[len] = *digit;
        len += 1;
    }

    header[len..len + HEADER_SUFFIX.len()].copy_from_slice(HEADER_SUFFIX);
    len += HEADER_SUFFIX.len();

    (header, len)
}

#[cfg(test)]
mod tests {
    use super::{advance_frame_offsets, content_length_header, write_frame};
    use std::{
        io::{self, IoSlice},
        pin::Pin,
        task::{Context, Poll},
    };
    use tokio::io::AsyncWrite;

    #[test]
    fn content_length_header_formats_zero_and_multi_digit_lengths() {
        for (length, expected) in [
            (0, "Content-Length: 0\r\n\r\n"),
            (42, "Content-Length: 42\r\n\r\n"),
            (123_456, "Content-Length: 123456\r\n\r\n"),
        ] {
            let (header, header_len) = content_length_header(length);

            assert_eq!(&header[..header_len], expected.as_bytes());
        }
    }

    #[test]
    fn content_length_header_formats_max_usize() {
        let expected = format!("Content-Length: {}\r\n\r\n", usize::MAX);

        let (header, header_len) = content_length_header(usize::MAX);

        assert_eq!(&header[..header_len], expected.as_bytes());
    }

    #[test]
    fn advance_frame_offsets_tracks_crossing_header_boundary() {
        let header = b"Content-Length: 2\r\n\r\n";
        let body = b"{}";
        let mut header_written = 0;
        let mut body_written = 0;

        advance_frame_offsets(
            header.len() + 1,
            header,
            body,
            &mut header_written,
            &mut body_written,
        )
        .expect("reported bytes fit within frame");

        assert_eq!(header_written, header.len());
        assert_eq!(body_written, 1);
    }

    #[test]
    fn advance_frame_offsets_rejects_impossible_byte_count() {
        let header = b"Content-Length: 2\r\n\r\n";
        let body = b"{}";
        let mut header_written = header.len();
        let mut body_written = 1;

        let error = advance_frame_offsets(2, header, body, &mut header_written, &mut body_written)
            .expect_err("reported bytes exceed remaining frame");

        assert_eq!(error.kind(), io::ErrorKind::InvalidData);
        assert_eq!(header_written, header.len());
        assert_eq!(body_written, 1);
    }

    #[tokio::test]
    async fn write_frame_uses_vectored_writes_for_partial_frame_progress() {
        let mut writer = RecordingWriter {
            max_write: 4,
            bytes: Vec::new(),
            scalar_writes: 0,
            vectored_writes: 0,
            supports_vectored: true,
        };

        write_frame(&mut writer, b"abc", b"defgh")
            .await
            .expect("frame should write");

        assert_eq!(writer.bytes, b"abcdefgh");
        assert_eq!(writer.scalar_writes, 0);
        assert!(writer.vectored_writes > 1);
    }

    #[tokio::test]
    async fn write_frame_falls_back_when_vectored_writes_are_not_supported() {
        let mut writer = RecordingWriter {
            max_write: 4,
            bytes: Vec::new(),
            scalar_writes: 0,
            vectored_writes: 0,
            supports_vectored: false,
        };

        write_frame(&mut writer, b"abc", b"defgh")
            .await
            .expect("frame should write");

        assert_eq!(writer.bytes, b"abcdefgh");
        assert!(writer.scalar_writes > 1);
        assert_eq!(writer.vectored_writes, 0);
    }

    struct RecordingWriter {
        max_write: usize,
        bytes: Vec<u8>,
        scalar_writes: usize,
        vectored_writes: usize,
        supports_vectored: bool,
    }

    impl AsyncWrite for RecordingWriter {
        fn poll_write(
            mut self: Pin<&mut Self>,
            _cx: &mut Context<'_>,
            buf: &[u8],
        ) -> Poll<io::Result<usize>> {
            self.scalar_writes += 1;
            let written = self.max_write.min(buf.len());
            self.bytes.extend_from_slice(&buf[..written]);
            Poll::Ready(Ok(written))
        }

        fn poll_flush(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<io::Result<()>> {
            Poll::Ready(Ok(()))
        }

        fn poll_shutdown(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<io::Result<()>> {
            Poll::Ready(Ok(()))
        }

        fn poll_write_vectored(
            mut self: Pin<&mut Self>,
            _cx: &mut Context<'_>,
            bufs: &[IoSlice<'_>],
        ) -> Poll<io::Result<usize>> {
            self.vectored_writes += 1;
            let mut remaining = self.max_write;
            let mut written = 0;

            for buf in bufs {
                if remaining == 0 {
                    break;
                }

                let chunk_len = remaining.min(buf.len());
                self.bytes.extend_from_slice(&buf[..chunk_len]);
                written += chunk_len;
                remaining -= chunk_len;
            }

            Poll::Ready(Ok(written))
        }

        fn is_write_vectored(&self) -> bool {
            self.supports_vectored
        }
    }
}
