use std::{net::{Ipv4Addr, Ipv6Addr, SocketAddrV4, SocketAddrV6}, str::FromStr};

use async_socket::AsyncSocket;
use socket2::{Domain, Protocol, SockAddr, Socket, Type};
use clap::Parser;
use tun::{Configuration, AbstractDevice};

mod async_socket;

#[derive(Parser)]
#[command(version, about, long_about = None)]
pub struct Args {
    #[arg(short, long)]
    pub src_addr: String,
    #[arg(short, long)]
    pub dst_addr: String,
}

fn convert_ethernet_frame_to_ether_packet(buf: &[u8]) -> Vec<u8> {
    let mut ether_header = vec![0; 16];
    // version
    ether_header[0] = 3 << 4;
    let mut packet = Vec::with_capacity(16 + buf.len());
    packet.extend_from_slice(&ether_header);
    packet.extend_from_slice(buf);
    packet
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    let (socket, dst_addr) = {
        let (domain, src_v4_addr, src_v6_addr) = if let Ok(addr) = Ipv4Addr::from_str(&args.src_addr) {
            (Domain::IPV4, Some(addr), None)
        } else {
            (Domain::IPV6, None, Some(Ipv6Addr::from_str(&args.src_addr).unwrap()))
        };
        let socket = Socket::new(domain, Type::RAW, Some(Protocol::from(97)))?;
        let dst_addr = if let Some(src_v4_addr) = src_v4_addr {
            socket.bind(&SockAddr::from(SocketAddrV4::new(src_v4_addr, 0)))?;
            SockAddr::from(SocketAddrV4::new(Ipv4Addr::from_str(&args.dst_addr).unwrap(), 0))
        } else if let Some(src_v6_addr) = src_v6_addr {
            socket.bind(&SockAddr::from(SocketAddrV6::new(src_v6_addr, 0, 0, 0)))?;
            SockAddr::from(SocketAddrV6::new(Ipv6Addr::from_str(&args.dst_addr).unwrap(), 0, 0, 0))
        } else {
            unreachable!()
        };
        (AsyncSocket::new(socket)?, dst_addr)
    };
    let device = {
        let mut config = Configuration::default();
        config.up();
        tun::create_as_async(&config)?
    };
    let size = device.mtu()? as usize + tun::PACKET_INFORMATION_LENGTH;
    loop {
        let mut buf = vec![0; size];
        let mut sbuf = vec![0; size];
        tokio::select! {
            n = device.recv(&mut buf) => {
                let n = n?;
                let packet = convert_ethernet_frame_to_ether_packet(&buf[..n]);
                let n = socket.send_to(&packet, dst_addr.clone()).await?;
                if n != buf.len() {
                    eprintln!("short write");
                }
            }
            n = socket.recv_from(&mut sbuf) => {
                let (n, _) = n?;
                let packet = sbuf[16..n].to_vec();
                let n = device.send(&packet).await?;
                if n != buf.len() {
                    eprintln!("short write");
                }
            }
        }
    }
    Ok(())
}
