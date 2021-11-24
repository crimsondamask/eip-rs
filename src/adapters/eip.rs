// rseip
//
// rseip - EIP&CIP in pure Rust.
// Copyright: 2021, Joylei <leingliu@gmail.com>
// License: MIT

use super::*;
use crate::{
    cip::codec::Encodable, cip::epath::EPATH_CONNECTION_MANAGER, cip::service::reply::*,
    cip::service::*, Result,
};
use rseip_eip::{EipContext, Frame};
use std::{convert::Infallible, io};
use tokio::io::{AsyncRead, AsyncWrite};

#[async_trait::async_trait(?Send)]
impl<T> Service for EipContext<T>
where
    T: AsyncRead + AsyncWrite + Unpin,
{
    /// context is open?
    fn is_open(&mut self) -> bool {
        self.session_handle().is_some()
    }

    /// open context
    async fn open(&mut self) -> Result<()> {
        if !self.has_session() {
            self.register_session().await?;
        }
        Ok(())
    }

    /// close context
    async fn close(&mut self) -> Result<()> {
        if self.has_session() {
            self.unregister_session().await?;
        }
        Ok(())
    }

    /// send Heartbeat message to keep underline transport alive
    #[inline]
    async fn heartbeat(&mut self) -> Result<()> {
        self.nop(Frame::<_, Infallible>::new(0, |_| Ok(()))).await?;
        Ok(())
    }

    /// send message router request without CIP connection
    #[inline]
    async fn unconnected_send<CP, P, D>(
        &mut self,
        request: UnconnectedSend<CP, MessageRequest<P, D>>,
    ) -> Result<MessageReply<Bytes>>
    where
        CP: Encodable,
        P: Encodable,
        D: Encodable,
    {
        let UnconnectedSend {
            priority_ticks,
            timeout_ticks,
            path: route_path,
            data: mr_data,
        } = request;
        let service_code = mr_data.service_code;
        let mr_data_len = mr_data.bytes_count();
        let path_len = route_path.bytes_count();

        assert!(mr_data_len <= u16::MAX as usize);
        debug_assert!(path_len % 2 == 0);
        assert!(path_len <= u8::MAX as usize);

        let unconnected_send: MessageRequest<&[u8], _> = MessageRequest {
            service_code: SERVICE_UNCONNECTED_SEND,
            path: EPATH_CONNECTION_MANAGER,
            data: LazyEncode {
                f: move |buf: &mut BytesMut| {
                    buf.put_u8(priority_ticks);
                    buf.put_u8(timeout_ticks);

                    buf.put_u16_le(mr_data_len as u16); // size of MR
                    mr_data.encode(buf)?;
                    if mr_data_len % 2 == 1 {
                        buf.put_u8(0); // padded 0
                    }

                    buf.put_u8(path_len as u8 / 2); // path size in words
                    buf.put_u8(0); // reserved
                    route_path.encode(buf)?; // padded epath
                    Ok(())
                },
                bytes_count: 4 + mr_data_len + mr_data_len % 2 + 2 + path_len,
            },
        };

        let frame = Frame::new(unconnected_send.bytes_count(), |buf| {
            unconnected_send
                .encode(buf)
                .map_err(|e| crate::Error::from(e))
        });

        let res: UnconnectedSendReply<Bytes> = self.send_rrdata(frame).await?;
        if res.0.reply_service != (service_code + 0x80) {
            return Err(io::Error::new(
                io::ErrorKind::Other,
                format!("unexpected reply service: {}", res.0.reply_service),
            )
            .into());
        }
        Ok(res.0)
    }

    /// send message router request with CIP explicit messaging connection
    #[inline]
    async fn connected_send<P, D>(
        &mut self,
        connection_id: u32,
        sequence_number: u16,
        request: MessageRequest<P, D>,
    ) -> Result<MessageReply<Bytes>>
    where
        P: Encodable,
        D: Encodable,
    {
        let service_code = request.service_code;
        let frame = Frame::new(request.bytes_count(), |buf| {
            request.encode(buf).map_err(|e| crate::Error::from(e))
        });
        let res: ConnectedSendReply<Bytes> = self
            .send_unit_data(connection_id, sequence_number, frame)
            .await?;
        if res.0.reply_service != (service_code + 0x80) {
            return Err(io::Error::new(
                io::ErrorKind::Other,
                format!("unexpected reply service: {}", res.0.reply_service),
            )
            .into());
        }
        Ok(res.0)
    }

    /// open CIP connection
    #[inline]
    async fn forward_open<P>(&mut self, request: Options<P>) -> Result<ForwardOpenReply>
    where
        P: Encodable,
    {
        let mr: MessageRequest<&[u8], _> = MessageRequest {
            service_code: SERVICE_FORWARD_OPEN,
            path: EPATH_CONNECTION_MANAGER,
            data: request,
        };
        let frame = Frame::new(mr.bytes_count(), |buf| {
            mr.encode(buf).map_err(|e| crate::Error::from(e))
        });
        let res: ForwardOpenReply = self.send_rrdata(frame).await?;
        Ok(res)
    }

    /// close CIP connection
    #[inline]
    async fn forward_close<P>(
        &mut self,
        request: ForwardCloseRequest<P>,
    ) -> Result<ForwardCloseReply>
    where
        P: Encodable,
    {
        let mr: MessageRequest<&[u8], _> = MessageRequest {
            service_code: SERVICE_FORWARD_CLOSE,
            path: EPATH_CONNECTION_MANAGER,
            data: request,
        };
        let frame = Frame::new(mr.bytes_count(), |buf| {
            mr.encode(buf).map_err(|e| crate::Error::from(e))
        });
        let res: ForwardCloseReply = self.send_rrdata(frame).await?;
        Ok(res)
    }
}
