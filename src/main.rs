use std::{
    net::{Ipv4Addr, Ipv6Addr, SocketAddrV4, SocketAddrV6},
    str::FromStr,
    sync::Arc,
};

use async_socket::AsyncSocket;
use bytes::BytesMut;
use clap::Parser;
use ipnet::IpNet;
use socket2::{Domain, Protocol, SockAddr, Socket, Type};
use tun::{AbstractDevice, AsyncDevice, Configuration, Layer};

mod async_socket;

#[derive(Parser)]
#[command(version, about, long_about = None)]
pub struct Args {
    #[arg(short, long)]
    pub src_addr: String,
    #[arg(short, long)]
    pub dst_addr: String,
    #[arg(long)]
    pub device_name: Option<String>,
    #[arg(short, long, default_value = "2")]
    pub threads: usize,
    #[arg(short, long)]
    pub address: Option<String>,
}

fn build_etherip_packet(buf: &[u8]) -> Vec<u8> {
    let mut ether_header = vec![0u8; 2];
    ether_header[0] = 3 << 4;
    let mut packet = Vec::with_capacity(2 + buf.len());
    packet.extend_from_slice(&ether_header);
    packet.extend_from_slice(buf);
    packet
}

async fn handle_device(
    size: usize,
    device: Arc<AsyncDevice>,
    socket: Arc<AsyncSocket>,
    dst_addr: SockAddr,
) -> anyhow::Result<()> {
    let mut buf = BytesMut::with_capacity(size);
    loop {
        buf.resize(size, 0);
        let n = device.recv(&mut buf).await?;
        buf.truncate(n);
        let packet = build_etherip_packet(&buf);
        socket.send_to(&packet, dst_addr.clone()).await?;
    }
    Ok(())
}

async fn handle_socket(
    size: usize,
    device: Arc<AsyncDevice>,
    socket: Arc<AsyncSocket>,
    dst_addr: SockAddr,
) -> anyhow::Result<()> {
    let mut sbuf = BytesMut::with_capacity(size);
    let is_ipv6 = dst_addr.is_ipv6();
    loop {
        sbuf.resize(size, 0);
        let (n, addr) = socket.recv_from(&mut sbuf).await?;
        if addr != dst_addr {
            continue;
        }
        sbuf.truncate(n);
        let ip_header_len = {
            if is_ipv6 {
                0
            } else {
                (sbuf[0] & 0x0F) as usize * 4
            }
        };
        if n < ip_header_len + 2 {
            tracing::error!("Received packet is too small: {} bytes", n);
            continue;
        }
        if sbuf[ip_header_len] >> 4 != 3 {
            tracing::error!(
                "Invalid EtherIP header: {:?}",
                &sbuf[ip_header_len..ip_header_len + 3]
            );
            continue;
        }
        let packet = sbuf.split_off(ip_header_len + 2);
        device.send(&packet).await?;
    }
    Ok(())
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();
    let args = Args::parse();
    let (socket, dst_addr) = {
        let (domain, src_v4_addr, src_v6_addr) =
            if let Ok(addr) = Ipv4Addr::from_str(&args.src_addr) {
                (Domain::IPV4, Some(addr), None)
            } else {
                (
                    Domain::IPV6,
                    None,
                    Some(Ipv6Addr::from_str(&args.src_addr).unwrap()),
                )
            };
        let socket = Socket::new(domain, Type::RAW, Some(Protocol::from(97)))?;
        socket.set_nonblocking(true)?;
        socket.set_reuse_address(true)?;
        let dst_addr = if let Some(src_v4_addr) = src_v4_addr {
            socket.bind(&SockAddr::from(SocketAddrV4::new(src_v4_addr, 0)))?;
            SockAddr::from(SocketAddrV4::new(
                Ipv4Addr::from_str(&args.dst_addr).unwrap(),
                0,
            ))
        } else if let Some(src_v6_addr) = src_v6_addr {
            socket.bind(&SockAddr::from(SocketAddrV6::new(src_v6_addr, 0, 0, 0)))?;
            SockAddr::from(SocketAddrV6::new(
                Ipv6Addr::from_str(&args.dst_addr).unwrap(),
                0,
                0,
                0,
            ))
        } else {
            return Err(anyhow::anyhow!("Invalid address"));
        };
        (Arc::new(AsyncSocket::new(socket)?), dst_addr)
    };
    let device = {
        let mut config = Configuration::default();
        config.platform_config(|c| {
            c.napi(true);
            c.vnet_hdr(false);
        });
        config.up();
        config.layer(Layer::L2);
        if let Some(device_name) = args.device_name {
            config.tun_name(device_name);
        }
        if let Some(address) = args.address {
            let addr = IpNet::from_str(&address)?;
            config.address(addr.addr());
            config.netmask(addr.netmask());
        }
        Arc::new(tun::create_as_async(&config)?)
    };
    let wait_handle_device = tokio::spawn(handle_device(
        (device.mtu()? + 50) as usize,
        Arc::clone(&device),
        Arc::clone(&socket),
        dst_addr.clone(),
    ));
    let wait_handle_socket = tokio::spawn(handle_socket(
        (device.mtu()? + 50) as usize,
        Arc::clone(&device),
        Arc::clone(&socket),
        dst_addr.clone(),
    ));

    tokio::select! {
        _ = tokio::signal::ctrl_c() => {},
        result = wait_handle_device => {
            tracing::error!("Device thread exited: {:?}", result);
        },
        result = wait_handle_socket => {
            tracing::error!("Socket thread exited: {:?}", result);
        },
    };
    Ok(())
}
