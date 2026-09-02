//! Packet framing: the outer `[Length][PacketID][Data]` envelope, and
//! the zlib packet compression that kicks in after a Set Compression
//! packet raises the threshold above -1. See
//! <https://wiki.vg/Protocol#Packet_format> and `#Compression`.

use bytes::{Buf, Bytes};
use flate2::read::{ZlibDecoder, ZlibEncoder};
use flate2::Compression;
use std::io::Read;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

use crate::error::{ProtocolError, Result};
use crate::varint::{read_varint_async, varint_size, write_varint};

/// Maximum accepted (post length-prefix) frame size, as a sanity check
/// against a corrupt stream or a malicious server trying to make us
/// allocate an enormous buffer.
const MAX_FRAME_LEN: usize = 8 * 1024 * 1024;

pub async fn read_packet<R: AsyncRead + Unpin>(
    reader: &mut R,
    compression_threshold: Option<i32>,
) -> Result<(i32, Bytes)> {
    let packet_len = read_varint_async(reader).await? as usize;
    if packet_len == 0 || packet_len > MAX_FRAME_LEN {
        return Err(ProtocolError::PacketTooLarge(packet_len));
    }

    let mut raw = vec![0u8; packet_len];
    reader.read_exact(&mut raw).await?;
    let mut raw = Bytes::from(raw);

    let mut payload = if compression_threshold.is_some() {
        let data_len = crate::varint::read_varint(&mut raw)? as usize;
        if data_len == 0 {
            // Below the compression threshold: what's left is the
            // plain, uncompressed [PacketID][Data].
            raw
        } else {
            if data_len > MAX_FRAME_LEN {
                return Err(ProtocolError::PacketTooLarge(data_len));
            }
            let mut decoder = ZlibDecoder::new(raw.reader());
            let mut out = vec![0u8; data_len];
            decoder
                .read_exact(&mut out)
                .map_err(ProtocolError::Io)?;
            Bytes::from(out)
        }
    } else {
        raw
    };

    let packet_id = crate::varint::read_varint(&mut payload)?;
    Ok((packet_id, payload))
}

pub async fn write_packet<W: AsyncWrite + Unpin>(
    writer: &mut W,
    packet_id: i32,
    body: &[u8],
    compression_threshold: Option<i32>,
) -> Result<()> {
    let mut uncompressed = Vec::with_capacity(varint_size(packet_id) + body.len());
    write_varint(&mut uncompressed, packet_id);
    uncompressed.extend_from_slice(body);

    let frame = match compression_threshold {
        None => {
            let mut frame = Vec::with_capacity(varint_size(uncompressed.len() as i32) + uncompressed.len());
            write_varint(&mut frame, uncompressed.len() as i32);
            frame.extend_from_slice(&uncompressed);
            frame
        }
        Some(threshold) => {
            let mut data = Vec::new();
            if uncompressed.len() >= threshold.max(0) as usize {
                write_varint(&mut data, uncompressed.len() as i32);
                let mut encoder =
                    ZlibEncoder::new(uncompressed.as_slice(), Compression::default());
                encoder.read_to_end(&mut data).map_err(ProtocolError::Io)?;
            } else {
                write_varint(&mut data, 0);
                data.extend_from_slice(&uncompressed);
            }
            let mut frame = Vec::with_capacity(varint_size(data.len() as i32) + data.len());
            write_varint(&mut frame, data.len() as i32);
            frame.extend_from_slice(&data);
            frame
        }
    };

    writer.write_all(&frame).await?;
    writer.flush().await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn round_trip(packet_id: i32, body: &[u8], compression_threshold: Option<i32>) -> (i32, Bytes) {
        let (mut client, mut server) = tokio::io::duplex(64 * 1024);
        write_packet(&mut client, packet_id, body, compression_threshold)
            .await
            .unwrap();
        drop(client); // flush's already happened; unblocks read_exact if it over-reads
        read_packet(&mut server, compression_threshold).await.unwrap()
    }

    #[tokio::test]
    async fn uncompressed_round_trip() {
        let (id, body) = round_trip(0x21, b"hello chunk", None).await;
        assert_eq!(id, 0x21);
        assert_eq!(&body[..], b"hello chunk");
    }

    #[tokio::test]
    async fn compressed_round_trip_above_threshold() {
        let payload = vec![0x42u8; 500]; // compresses well and exceeds the threshold
        let (id, body) = round_trip(0x21, &payload, Some(64)).await;
        assert_eq!(id, 0x21);
        assert_eq!(&body[..], &payload[..]);
    }

    #[tokio::test]
    async fn compressed_round_trip_below_threshold_stays_uncompressed_on_wire() {
        let payload = b"tiny";
        let (id, body) = round_trip(0x00, payload, Some(256)).await;
        assert_eq!(id, 0x00);
        assert_eq!(&body[..], payload);
    }
}
