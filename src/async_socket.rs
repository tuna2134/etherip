use std::io;

use socket2::{SockAddr, Socket};
use tokio::io::{unix::AsyncFd, ReadBuf};

pub struct AsyncSocket {
    inner: AsyncFd<Socket>,
}

impl AsyncSocket {
    pub fn new(socket: Socket) -> anyhow::Result<Self> {
        let inner = AsyncFd::new(socket)?;
        Ok(Self { inner })
    }

    pub async fn recv_from(&self, buf: &mut [u8]) -> Result<(usize, SockAddr), io::Error> {
        loop {
            let mut guard = self.inner.readable().await?;

            match guard.try_io(|inner| {
                let mut maybeuninit_buf = ReadBuf::new(buf);
                let x = unsafe { maybeuninit_buf.unfilled_mut() };
                let y = inner.get_ref().recv_from(x);
                let filled = maybeuninit_buf.filled_mut();
                y
            }) {
                Ok(x) => return x,
                Err(_would_block) => continue,
            }
        }
    }

    pub async fn send_to(&self, buf: &[u8], addr: SockAddr) -> Result<usize, io::Error> {
        loop {
            let mut guard = self.inner.writable().await?;

            match guard.try_io(|inner| inner.get_ref().send_to(buf, &addr)) {
                Ok(x) => return x,
                Err(_would_block) => continue,
            }
        }
    }
}