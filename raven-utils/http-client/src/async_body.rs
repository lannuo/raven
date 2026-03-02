use std::{
    io::{Cursor, Read},
    pin::Pin,
    task::Poll,
};

use bytes::Bytes;
use futures::AsyncRead;
use http_body::{Body, Frame};

pub struct AsyncBody(pub Inner);

pub enum Inner {
    /// 空Body
    Empty,

    /// 内存存储
    Bytes(Cursor<Bytes>),

    /// 异步读取
    AsyncReader(Pin<Box<dyn AsyncRead + Send + Sync>>),
}

impl AsyncBody {
    /// 创建一个空的异步体
    pub fn empty() -> Self {
        Self(Inner::Empty)
    }

    /// 从字节切片创建异步体
    pub fn from_bytes(bytes: Bytes) -> Self {
        Self(Inner::Bytes(Cursor::new(bytes)))
    }

    /// 从异步读取器创建异步体
    pub fn from_reader<R>(read: R) -> Self
    where
        R: AsyncRead + Send + Sync + 'static,
    {
        Self(Inner::AsyncReader(Box::pin(read)))
    }
}

impl Default for AsyncBody {
    fn default() -> Self {
        Self(Inner::Empty)
    }
}

impl From<()> for AsyncBody {
    fn from(_: ()) -> Self {
        Self(Inner::Empty)
    }
}

impl From<Bytes> for AsyncBody {
    fn from(bytes: Bytes) -> Self {
        Self::from_bytes(bytes)
    }
}

impl From<Vec<u8>> for AsyncBody {
    fn from(body: Vec<u8>) -> Self {
        Self::from_bytes(body.into())
    }
}

impl From<String> for AsyncBody {
    fn from(body: String) -> Self {
        Self::from_bytes(body.into())
    }
}

impl From<&'static [u8]> for AsyncBody {
    #[inline]
    fn from(s: &'static [u8]) -> Self {
        Self::from_bytes(Bytes::from_static(s))
    }
}

impl From<&'static str> for AsyncBody {
    #[inline]
    fn from(body: &'static str) -> Self {
        Self::from_bytes(Bytes::from_static(body.as_bytes()))
    }
}

impl<T: Into<Self>> From<Option<T>> for AsyncBody {
    fn from(body: Option<T>) -> Self {
        match body {
            Some(body) => body.into(),
            None => Self::empty(),
        }
    }
}

impl AsyncRead for AsyncBody {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &mut [u8],
    ) -> std::task::Poll<std::io::Result<usize>> {
        // SAFETY: Standard Enum pin projection
        let inner = unsafe { &mut self.get_unchecked_mut().0 };
        match inner {
            Inner::Empty => Poll::Ready(Ok(0)),
            // Blocking call is over an in-memory buffer
            Inner::Bytes(cursor) => Poll::Ready(cursor.read(buf)),
            Inner::AsyncReader(async_reader) => {
                AsyncRead::poll_read(async_reader.as_mut(), cx, buf)
            }
        }
    }
}

impl Body for AsyncBody {
    type Data = Bytes;
    type Error = std::io::Error;

    fn poll_frame(
        mut self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> Poll<Option<Result<Frame<Self::Data>, Self::Error>>> {
        let mut buffer = vec![0; 8192];
        match AsyncRead::poll_read(self.as_mut(), cx, &mut buffer) {
            Poll::Ready(Ok(0)) => Poll::Ready(None),
            Poll::Ready(Ok(n)) => {
                let data = Bytes::copy_from_slice(&buffer[..n]);
                Poll::Ready(Some(Ok(Frame::data(data))))
            }
            Poll::Ready(Err(e)) => Poll::Ready(Some(Err(e))),
            Poll::Pending => Poll::Pending,
        }
    }
}
