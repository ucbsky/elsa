use bridge::id_tracker::{IdGen, RecvId, SendId};
use crypto_primitives::malpriv::MessageHash;
use tracing::{error, info};

/// Message IDs for various clients
pub struct IdPool {
    pub otverify_a: Vec<RecvId>,
    pub otverify_b: Vec<SendId>,

    pub b2a_a: Vec<SendId>,
    pub b2a_b: Vec<RecvId>,
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

        IdPool {
            otverify_a,
            otverify_b,
            b2a_a,
            b2a_b,
        }
    }
}

pub struct HashPool<H: MessageHash> {
    pub b2a_ab: Vec<H>,
    pub ot_ba: Vec<H>,
}

impl<H: MessageHash> HashPool<H> {
    pub fn init(alice_pool_size: usize, bob_pool_size: usize, hasher: impl Fn() -> H) -> Self {
        let hasher = |_| hasher();
        let b2a_ab = (0..bob_pool_size).map(hasher).collect::<Vec<_>>();
        let ot_ba = (0..alice_pool_size).map(hasher).collect::<Vec<_>>();

        Self { b2a_ab, ot_ba }
    }
}
#[inline]
pub fn log_verify_status(num_verified: usize, num_total: usize, name: &str) {
    if num_verified == num_total {
        info!("[{}] All passed!", name);
    } else {
        error!(
            "[{}] # successful verifications: {}/{}",
            name, num_verified, num_total
        );
    }
}
