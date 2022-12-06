use bridge::id_tracker::{ExchangeId, IdGen, RecvId, SendId};

/// Message IDs for various clients
pub struct IdPool {
    pub otverify_a: Vec<RecvId>,
    pub otverify_b: Vec<SendId>,

    pub b2a_a: Vec<SendId>,
    pub b2a_b: Vec<RecvId>,

    pub sqcorr: Vec<(ExchangeId, ExchangeId)>,

    pub a2s: Vec<ExchangeId>,
}

impl IdPool {
    pub fn build(alice_pool_size: usize, bob_pool_size: usize) -> Self {
        // manage message ids
        // for now, denote `a` as Alice (OT Sender) and `b` as Bob (OT Receiver)

        let mut id = IdGen::new();

        let otverify_a = (0..alice_pool_size)
            .map(|_| id.next_recv_id())
            .collect::<Vec<_>>();
        let otverify_b = (0..bob_pool_size)
            .map(|_| id.next_send_id())
            .collect::<Vec<_>>();

        let b2a_a = (0..alice_pool_size)
            .map(|_| id.next_send_id())
            .collect::<Vec<_>>();
        let b2a_b = (0..bob_pool_size)
            .map(|_| id.next_recv_id())
            .collect::<Vec<_>>();

        let sqcorr = (0..alice_pool_size + bob_pool_size)
            .map(|_| (id.next_exchange_id(), id.next_exchange_id()))
            .collect::<Vec<_>>();

        let a2s = (0..alice_pool_size + bob_pool_size)
            .map(|_| id.next_exchange_id())
            .collect::<Vec<_>>();

        IdPool {
            otverify_a,
            otverify_b,
            b2a_a,
            b2a_b,
            sqcorr,
            a2s,
        }
    }
}
