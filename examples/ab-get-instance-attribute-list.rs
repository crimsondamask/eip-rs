// rseip
//
// rseip - EIP&CIP in pure Rust.
// Copyright: 2021, Joylei <leingliu@gmail.com>
// License: MIT

use anyhow::Result;
use futures::StreamExt;
use rseip::{
    cip::{epath::PortSegment, service::MessageService},
    client::{AbEipClient, AbService},
};

#[tokio::main]
pub async fn main() -> Result<()> {
    let mut client = AbEipClient::new_host_lookup("192.168.0.83")
        .await?
        .with_connection_path(PortSegment::default());
    {
        let stream = client.get_instance_attribute_list().call();
        stream
            .for_each(|item| async move {
                println!("{:?}", item);
            })
            .await;
    }
    client.close().await?;
    Ok(())
}
