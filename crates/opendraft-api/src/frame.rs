//! Length-delimited frames for the local transport: a big-endian `u32` byte
//! count, then that many bytes. The length is checked before anything is
//! allocated, so a hostile peer cannot make the reader reserve gigabytes.

use std::io::{self, Read, Write};

use crate::limits::IPC_FRAME_BYTES;

#[derive(Debug)]
pub enum FrameError {
    /// The peer closed the connection between frames.
    Closed,
    /// The announced length is over [`IPC_FRAME_BYTES`].
    TooLarge(usize),
    Io(io::Error),
}

impl From<io::Error> for FrameError {
    fn from(error: io::Error) -> Self {
        FrameError::Io(error)
    }
}

pub fn write_frame<W: Write>(out: &mut W, payload: &[u8]) -> Result<(), FrameError> {
    if payload.len() > IPC_FRAME_BYTES {
        return Err(FrameError::TooLarge(payload.len()));
    }
    // Checked above: IPC_FRAME_BYTES fits a u32.
    let length = u32::try_from(payload.len()).map_err(|_| FrameError::TooLarge(payload.len()))?;
    out.write_all(&length.to_be_bytes())?;
    out.write_all(payload)?;
    out.flush()?;
    Ok(())
}

pub fn read_frame<R: Read>(input: &mut R) -> Result<Vec<u8>, FrameError> {
    let mut header = [0u8; 4];
    match input.read_exact(&mut header) {
        Ok(()) => {}
        Err(e) if e.kind() == io::ErrorKind::UnexpectedEof => return Err(FrameError::Closed),
        Err(e) => return Err(e.into()),
    }
    let length = u32::from_be_bytes(header) as usize;
    if length > IPC_FRAME_BYTES {
        return Err(FrameError::TooLarge(length));
    }
    let mut payload = vec![0u8; length];
    input.read_exact(&mut payload)?;
    Ok(payload)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_frame_round_trips() {
        let mut wire = Vec::new();
        write_frame(&mut wire, b"hello").unwrap();
        write_frame(&mut wire, b"").unwrap();
        let mut input = wire.as_slice();
        assert_eq!(read_frame(&mut input).unwrap(), b"hello");
        assert_eq!(read_frame(&mut input).unwrap(), b"");
        assert!(matches!(read_frame(&mut input), Err(FrameError::Closed)));
    }

    #[test]
    fn an_oversized_length_is_refused_before_reading_it() {
        let wire = u32::MAX.to_be_bytes();
        assert!(matches!(
            read_frame(&mut wire.as_slice()),
            Err(FrameError::TooLarge(_))
        ));
    }

    #[test]
    fn a_truncated_frame_is_an_error_not_a_hang() {
        let mut wire = Vec::new();
        write_frame(&mut wire, b"hello").unwrap();
        wire.truncate(6);
        assert!(matches!(
            read_frame(&mut wire.as_slice()),
            Err(FrameError::Io(_))
        ));
    }
}
