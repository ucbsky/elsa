//! async protocols

use bridge::{id_tracker::SendId, tcp_bridge::TcpConnection};
use crypto_primitives::{
    bits::{batch_make_boolean_shares, BitsLE, SeededInputShare},
    uint::UInt,
};
use rand::Rng;
use serialize::AsUseCast;
use tokio::sync::oneshot;

#[derive(Debug, Clone)]
pub struct Client<T: UInt> {
    pub prepared_message_0: SeededInputShare,
    pub prepared_message_1: Vec<BitsLE<T>>,
}

impl<T: UInt> Client<T> {
    pub fn new<R: Rng>(input: &[T], rng: &mut R) -> Self {
        let (input_0, input_1) = batch_make_boolean_shares(rng, input.iter().map(|x| x.bits_le()));

        Self {
            prepared_message_0: input_0,
            prepared_message_1: input_1,
        }
    }

    pub fn send_to_server_0(&self, id: SendId, conn: TcpConnection) -> oneshot::Receiver<()> {
        if conn.uid().is_even() {
            conn.send_message(id, &self.prepared_message_0.use_cast())
                .unwrap()
        } else {
            conn.send_message(id, &self.prepared_message_1).unwrap()
        }
    }

    pub fn send_to_server_1(&self, id: SendId, conn: TcpConnection) -> oneshot::Receiver<()> {
        if conn.uid().is_even() {
            conn.send_message(id, &self.prepared_message_1).unwrap()
        } else {
            conn.send_message(id, &self.prepared_message_0.use_cast())
                .unwrap()
        }
    }
}
