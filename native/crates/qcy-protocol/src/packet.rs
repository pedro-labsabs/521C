use crate::SOF;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandBlock {
    pub cmd: u8,
    pub params: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Packet {
    pub blocks: Vec<CommandBlock>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DecodeError {
    TooShort,
    BadSof,
    LengthMismatch,
    TruncatedBlock,
    Oversize,
}

const MAX_PACKET: usize = 512;

pub fn encode_blocks(blocks: &[CommandBlock]) -> Result<Vec<u8>, DecodeError> {
    let mut body = Vec::new();
    for b in blocks {
        if b.params.len() > 255 {
            return Err(DecodeError::Oversize);
        }
        body.push(b.cmd);
        body.push(b.params.len() as u8);
        body.extend_from_slice(&b.params);
    }
    if body.len() > 255 {
        return Err(DecodeError::Oversize);
    }
    let mut out = Vec::with_capacity(2 + body.len());
    out.push(SOF);
    out.push(body.len() as u8);
    out.extend(body);
    Ok(out)
}

pub fn encode_command(cmd: u8, params: &[u8]) -> Result<Vec<u8>, DecodeError> {
    encode_blocks(&[CommandBlock {
        cmd,
        params: params.to_vec(),
    }])
}

pub fn decode_packet(data: &[u8]) -> Result<Packet, DecodeError> {
    if data.len() < 2 {
        return Err(DecodeError::TooShort);
    }
    if data.len() > MAX_PACKET {
        return Err(DecodeError::Oversize);
    }
    if data[0] != SOF {
        return Err(DecodeError::BadSof);
    }
    let body_len = data[1] as usize;
    if data.len() != body_len + 2 {
        return Err(DecodeError::LengthMismatch);
    }
    let mut blocks = Vec::new();
    let mut i = 2;
    while i < data.len() {
        if i + 2 > data.len() {
            return Err(DecodeError::TruncatedBlock);
        }
        let cmd = data[i];
        let param_len = data[i + 1] as usize;
        if i + 2 + param_len > data.len() {
            return Err(DecodeError::TruncatedBlock);
        }
        blocks.push(CommandBlock {
            cmd,
            params: data[i + 2..i + 2 + param_len].to_vec(),
        });
        i += 2 + param_len;
    }
    Ok(Packet { blocks })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn battery_request_roundtrip() {
        let pkt = encode_command(0xFE, &[0x2F]).unwrap();
        assert_eq!(pkt, vec![0xFF, 0x03, 0xFE, 0x01, 0x2F]);
        let decoded = decode_packet(&pkt).unwrap();
        assert_eq!(decoded.blocks[0].cmd, 0xFE);
        assert_eq!(decoded.blocks[0].params, vec![0x2F]);
    }

    #[test]
    fn rejects_bad_sof() {
        assert_eq!(decode_packet(&[0x00, 0x01, 0x00]), Err(DecodeError::BadSof));
    }

    #[test]
    fn rejects_length_mismatch() {
        assert_eq!(
            decode_packet(&[0xFF, 0x10, 0x01]),
            Err(DecodeError::LengthMismatch)
        );
    }

    #[test]
    fn rejects_truncated_block() {
        assert_eq!(
            decode_packet(&[0xFF, 0x03, 0x2F, 0x05, 0x00]),
            Err(DecodeError::TruncatedBlock)
        );
    }

    #[test]
    fn multi_block() {
        let pkt = encode_blocks(&[
            CommandBlock {
                cmd: 0x2F,
                params: vec![0x52, 0x50, 0x5E],
            },
            CommandBlock {
                cmd: 0x09,
                params: vec![0x01],
            },
        ])
        .unwrap();
        let d = decode_packet(&pkt).unwrap();
        assert_eq!(d.blocks.len(), 2);
        assert_eq!(d.blocks[1].cmd, 0x09);
    }
}
