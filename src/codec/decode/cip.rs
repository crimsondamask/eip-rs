use crate::{
    error::{Error, ResponseError},
    frame::cip::{MessageRouterReply, Status},
};
use byteorder::{ByteOrder, LittleEndian};
use bytes::Bytes;
use std::convert::TryFrom;

impl TryFrom<Bytes> for MessageRouterReply<Bytes> {
    type Error = Error;

    #[inline]
    fn try_from(buf: Bytes) -> Result<Self, Self::Error> {
        if buf.len() < 4 {
            return Err(Error::Response(ResponseError::InvalidData));
        }
        let reply_service = buf[0];
        //reserved buf[1]
        let general_status = buf[2];
        let extended_status_size = buf[3];
        if buf.len() < 4 + extended_status_size as usize {
            return Err(Error::Response(ResponseError::InvalidData));
        }
        let extended_status = match extended_status_size {
            0 => 0,
            1 => buf[4] as u16,
            2 => LittleEndian::read_u16(&buf[4..6]),
            _ => return Err(Error::Response(ResponseError::InvalidData)),
        };
        let status = Status {
            general: general_status,
            extended: extended_status,
        };
        if general_status != 0 {
            return Err(Error::CIPError(status));
        }
        let data = buf.slice(4 + extended_status_size as usize..);
        Ok(Self::new(reply_service, status, data))
    }
}
