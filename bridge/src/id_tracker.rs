use std::fmt::{self, Display, Formatter};

use bytemuck::{Pod, Zeroable};

/// Message ID used to send
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Pod, Zeroable)]
#[repr(transparent)]
pub struct SendId(pub u64);

impl SendId {
    pub const FIRST: Self = SendId(COMMON_MESSAGE_ID_START);
    pub const SECOND: Self = SendId(COMMON_MESSAGE_ID_START + 1);
    pub const THIRD: Self = SendId(COMMON_MESSAGE_ID_START + 2);
}

impl From<u64> for SendId {
    fn from(id: u64) -> Self {
        SendId(id)
    }
}

impl Display for SendId {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        write!(f, "send({})", self.0)
    }
}
/// Message ID used to receive
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Pod, Zeroable)]
#[repr(transparent)]
pub struct RecvId(pub u64);

impl RecvId {
    pub const FIRST: Self = RecvId(COMMON_MESSAGE_ID_START);
    pub const SECOND: Self = RecvId(COMMON_MESSAGE_ID_START + 1);
    pub const THIRD: Self = RecvId(COMMON_MESSAGE_ID_START + 2);
}

impl From<u64> for RecvId {
    fn from(id: u64) -> Self {
        RecvId(id)
    }
}

impl Display for RecvId {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        write!(f, "recv({})", self.0)
    }
}

/// Message ID used to exchange
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ExchangeId {
    pub send_id: SendId,
    pub recv_id: RecvId,
}
impl From<(u64, u64)> for ExchangeId {
    fn from(id: (u64, u64)) -> Self {
        ExchangeId {
            send_id: SendId(id.0),
            recv_id: RecvId(id.1),
        }
    }
}
impl From<(SendId, RecvId)> for ExchangeId {
    fn from(id: (SendId, RecvId)) -> Self {
        ExchangeId {
            send_id: id.0,
            recv_id: id.1,
        }
    }
}
impl From<u64> for ExchangeId {
    fn from(id: u64) -> Self {
        ExchangeId {
            send_id: SendId(id),
            recv_id: RecvId(id),
        }
    }
}

/// message id 0 is reserved for register message
pub const REGISTER_MESSAGE_ID: u64 = 0;
pub const COMMON_MESSAGE_ID_START: u64 = 1;

/// Used to generate a new message ID for each message to be sent or received.
/// Starting from 0.
#[derive(Debug)]
pub struct IdGen {
    next_send_id: u64,
    next_recv_id: u64,
    next_send_id_bound: u64,
    next_recv_id_bound: u64,
}

impl IdGen {
    pub fn new() -> Self {
        Self {
            next_send_id: COMMON_MESSAGE_ID_START,
            next_recv_id: COMMON_MESSAGE_ID_START,
            next_send_id_bound: u64::MAX,
            next_recv_id_bound: u64::MAX,
        }
    }

    pub fn next_send_id(&mut self) -> SendId {
        if self.next_send_id == self.next_send_id_bound {
            panic!("sending too many messages than expected")
        }
        let id = self.next_send_id;
        self.next_send_id += 1;
        id.into()
    }

    pub fn next_recv_id(&mut self) -> RecvId {
        if self.next_recv_id == self.next_recv_id_bound {
            panic!("receiving too many messages than expected")
        }
        let id = self.next_recv_id;
        self.next_recv_id += 1;
        id.into()
    }

    pub fn next_exchange_id(&mut self) -> ExchangeId {
        let id = (self.next_send_id(), self.next_recv_id());
        id.into()
    }

    /// Reserve a range of IDs for message exchange. It will return a new IdGen
    /// that can only send/receive `num_rounds` messages. For current IdGen,
    /// `next_send_id` and `next_recv_id` will advance by `num_rounds`.
    pub fn reserve_rounds(&mut self, num_rounds: u64) -> Self {
        let reserved = Self {
            next_recv_id: self.next_recv_id,
            next_send_id: self.next_send_id,
            next_recv_id_bound: self.next_recv_id + num_rounds,
            next_send_id_bound: self.next_send_id + num_rounds,
        };
        self.next_recv_id += num_rounds;
        self.next_send_id += num_rounds;
        reserved
    }
}
