// rseip
//
// rseip - EIP&CIP in pure Rust.
// Copyright: 2021, Joylei <leingliu@gmail.com>
// License: MIT

use crate::{CommonPacket, Error, MessageReply, Result};
use byteorder::{ByteOrder, LittleEndian};
use bytes::Bytes;
use std::convert::TryFrom;
use std::io;

#[derive(Debug)]
pub struct ConnectedSendReply<D>(pub MessageReply<D>);

impl TryFrom<CommonPacket> for ConnectedSendReply<Bytes> {
    type Error = Error;
    #[inline]
    fn try_from(cpf: CommonPacket) -> Result<Self> {
        let mut cpf = cpf.into_inner();
        if cpf.len() != 2 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "common packet -  expected 2 items",
            )
            .into());
        }
        // should be connected address
        cpf[0].ensure_type_code(0xA1)?;
        let data_item = cpf.remove(1);
        // should be unconnected data item
        data_item.ensure_type_code(0xB1)?;
        if data_item.data.len() < 2 {
            if cpf.len() != 2 {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "CIP - failed to decode message reply",
                )
                .into());
            }
        }

        //TODO: validate sequence count
        let _sequence_count = LittleEndian::read_u16(&data_item.data[0..2]);
        let mr_reply = MessageReply::try_from(data_item.data.slice(2..))?;
        Ok(Self(mr_reply))
    }
}

#[derive(Debug)]
pub struct UnconnectedSendReply<D>(pub MessageReply<D>);

impl TryFrom<CommonPacket> for UnconnectedSendReply<Bytes> {
    type Error = Error;
    #[inline]
    fn try_from(cpf: CommonPacket) -> Result<Self> {
        let mut cpf = cpf.into_inner();
        if cpf.len() != 2 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "common packet -  expected 2 items",
            )
            .into());
        }
        // should be null address
        cpf[0].ensure_type_code(0)?;
        let data_item = cpf.remove(1);
        // should be unconnected data item
        data_item.ensure_type_code(0xB2)?;
        let mr_reply = MessageReply::try_from(data_item.data)?;
        Ok(Self(mr_reply))
    }
}

#[derive(Debug)]
pub struct AttributeReply {
    pub id: u16,
    pub status: u16,
    pub data: Bytes,
}