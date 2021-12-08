// rseip
//
// rseip - EIP&CIP in pure Rust.
// Copyright: 2021, Joylei <leingliu@gmail.com>
// License: MIT

use crate::service::*;
use crate::*;
use crate::{epath::EPath, error::cip_error};
use bytes::{Buf, BufMut, Bytes, BytesMut};
use rseip_core::codec::{BytesHolder, Decode, Decoder, Encode, Encoder, LittleEndianDecoder};
use smallvec::SmallVec;

/// build and send multiple service packet
pub struct MultipleServicePacket<'a, T, P, D> {
    inner: &'a mut T,
    items: SmallVec<[MessageRequest<P, D>; 8]>,
}

impl<'a, T, P, D> MultipleServicePacket<'a, T, P, D> {
    pub(crate) fn new(inner: &'a mut T) -> Self {
        Self {
            inner,
            items: Default::default(),
        }
    }
}

impl<'a, T, P, D> MultipleServicePacket<'a, T, P, D>
where
    T: MessageService,
    P: Encode,
    D: Encode,
{
    /// append service request
    pub fn push(mut self, mr: MessageRequest<P, D>) -> Self {
        self.items.push(mr);
        self
    }

    /// append all service requests
    pub fn push_all(mut self, items: impl Iterator<Item = MessageRequest<P, D>>) -> Self
    where
        P: Encode + 'static,
        D: Encode + 'static,
    {
        for mr in items {
            self.items.push(mr);
        }
        self
    }

    /// build and send requests
    pub async fn call(self) -> Result<ReplyIter<LittleEndianDecoder<T::Error>>, T::Error> {
        let Self { inner, items } = self;
        if items.is_empty() {
            return Ok(ReplyIter::new(None));
        }

        let req = MessageRequest {
            service_code: 0x0A,
            path: EPath::default().with_class(2).with_instance(1),
            data: MultipleServicesEncoder { items },
        };
        let reply: MessageReply<BytesHolder> = inner.send(req).await?;

        if reply.reply_service != 0x8A {
            return Err(cip_error(format_args!(
                "unexpected reply service for multiple service packet: {:#0x}",
                reply.reply_service
            )));
        }

        let res = ReplyIter::new(Some(LittleEndianDecoder::new(reply.data.into())));
        Ok(res)
    }
}

pub struct ReplyIter<D> {
    buf: Option<D>,
    offsets: Bytes,
    count: Option<u16>,
    last: Option<u16>,
    i: u16,
}

impl<D> ReplyIter<D> {
    fn new(decoder: Option<D>) -> Self {
        Self {
            buf: decoder,
            offsets: Bytes::new(),
            count: None,
            last: None,
            i: 0,
        }
    }
}

impl<'de, D> ReplyIter<D>
where
    D: Decoder<'de>,
{
    #[inline]
    fn raise_err<T>(&mut self) -> Option<Result<T, D::Error>> {
        self.buf.take();
        Some(Err(cip_error("failed to decode message reply")))
    }

    /// decode next message reply from the multiple service reply
    pub fn next<Item>(&mut self) -> Option<Result<MessageReply<Item>, D::Error>>
    where
        Item: Decode<'de>,
    {
        let buf = self.buf.as_mut()?;
        let count = if let Some(count) = self.count {
            count
        } else {
            let count = buf.decode_u16();
            self.count = Some(count);
            if count == 0 {
                return None;
            }
            let data_offsets = 2 * (count) as usize;
            if let Err(e) = buf.ensure_size(data_offsets) {
                return Some(Err(e));
            }
            self.offsets = buf.buf_mut().copy_to_bytes(data_offsets);
            count
        };
        if count == 0 {
            return None;
        }

        while self.i < count {
            self.i += 1;
            let offset = self.offsets.get_u16_le();
            if let Some(last) = self.last.replace(offset) {
                if offset <= last {
                    return self.raise_err();
                }
                let size = (offset - last) as usize;
                if buf.remaining() < size {
                    return self.raise_err();
                }
                let res: Result<MessageReply<Item>, _> = buf.decode_any();
                return Some(res);
            }
        }
        // process remaining
        if buf.remaining() > 0 {
            let res: Result<MessageReply<Item>, _> = buf.decode_any();
            self.buf.take();
            return Some(res);
        }
        self.buf.take();
        None
    }
}

/// array encoder, expected array size in u16
struct MultipleServicesEncoder<Array>
where
    Array: smallvec::Array,
{
    items: SmallVec<Array>,
}

impl<Array> MultipleServicesEncoder<Array>
where
    Array: smallvec::Array,
    Array::Item: Encode,
{
    #[inline]
    fn encode_common<A: Encoder>(
        &self,
        buf: &mut BytesMut,
        _encoder: &mut A,
    ) -> StdResult<(), A::Error>
    where
        Self: Sized,
    {
        let start_offset = 2 + 2 * self.items.len();
        buf.put_u16_le(self.items.len() as u16);
        let mut offset = start_offset;
        for item in self.items.iter() {
            buf.put_u16_le(offset as u16);
            offset += item.bytes_count();
        }
        Ok(())
    }
}

impl<Array> Encode for MultipleServicesEncoder<Array>
where
    Array: smallvec::Array,
    Array::Item: Encode,
{
    #[inline]
    fn encode<A: Encoder>(self, buf: &mut BytesMut, encoder: &mut A) -> StdResult<(), A::Error>
    where
        Self: Sized,
    {
        self.encode_common(buf, encoder)?;
        for item in self.items {
            item.encode(buf, encoder)?;
        }
        Ok(())
    }

    #[inline]
    fn encode_by_ref<A: rseip_core::codec::Encoder>(
        &self,
        buf: &mut BytesMut,
        encoder: &mut A,
    ) -> StdResult<(), A::Error> {
        self.encode_common(buf, encoder)?;
        for item in self.items.iter() {
            item.encode_by_ref(buf, encoder)?;
        }
        Ok(())
    }

    #[inline]
    fn bytes_count(&self) -> usize {
        let start_offset = 2 + 2 * self.items.len();
        let bytes_count = self.items.iter().map(|v| v.bytes_count()).sum::<usize>();
        start_offset + bytes_count
    }
}
