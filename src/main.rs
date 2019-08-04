#![feature(async_await)]

mod ssclient;
mod tun;

use crate::tun::socket::TunSocket;
use crate::tun::{bg_send, listen};
use log::debug;
use shadowsocks::crypto::CipherType;
use shadowsocks::relay::tcprelay::{DecryptedRead, EncryptedWrite};
use shadowsocks::{ServerAddr, ServerConfig};
use smoltcp::wire::IpAddress;
use smoltcp::wire::IpEndpoint;
use std::collections::HashMap;
use std::env;
use std::io;
use std::io::Read;
use std::net::{Ipv4Addr, SocketAddr};
use std::time::Duration;
use tokio::prelude::future::lazy;
use tokio::prelude::{AsyncRead, AsyncSink, AsyncWrite, Future, IntoFuture, Sink, Stream, Write};
use tokio::runtime::current_thread::{block_on_all, run, spawn};
use tokio::sync::mpsc::channel;

fn main() -> io::Result<()> {
    env_logger::init();
    better_panic::install();

    let args = env::args().collect::<Vec<String>>();
    let _name = &args[1];
    //
    //    let srv_cfg = ServerConfig::new(
    //        ServerAddr::DomainName("127.0.0.1".to_string(), 14187),
    //        "rixCloud".to_string(),
    //        CipherType::ChaCha20Ietf,
    //        Some(Duration::from_secs(30)),
    //        None,
    //    );

    run(lazy(|| {
        spawn(bg_send().map_err(|_| ()));
        listen()
            .for_each(|mut socket| {
                spawn(lazy(move || -> Box<dyn Future<Item = (), Error = ()>> {
                    match socket {
                        TunSocket::Tcp(socket) => {
                            let (reader, writer) = socket.split();
                            Box::new(tokio::io::copy(reader, writer).map(|_| ()).map_err(|_| ()))
                        }
                        TunSocket::Udp(socket) => {
                            let mut buf = vec![0; 1000];
                            Box::new(
                                socket
                                    .recv_dgram(buf)
                                    .and_then(|(socket, mut buf, size, addr)| {
                                        buf.truncate(size);
                                        socket.send_dgram(buf, addr)
                                    })
                                    .map(|_| ())
                                    .map_err(|_| ()),
                            )
                        }
                    }
                }));
                Ok(())
            })
            .map_err(|_| ())
    }));

    Ok(())
}

fn to_socket_addr(endpoint: IpEndpoint) -> SocketAddr {
    match endpoint.addr {
        IpAddress::Ipv4(addr) => {
            let a: Ipv4Addr = addr.into();
            (a, endpoint.port).into()
        }
        _ => unreachable!(),
    }
}
