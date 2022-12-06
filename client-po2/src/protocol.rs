use bin_utils::client::Options;
use bridge::{
    client_server::init_meta_clients, end_timer, id_tracker::SendId, start_timer,
    tcp_bridge::TcpConnection,
};
use crypto_primitives::{
    bits::batch_make_boolean_shares,
    cot::client::{num_additional_ot_needed, COTGen},
    message::po2::{ClientPo2MsgToAlice, ClientPo2MsgToBob},
    uint::UInt,
};
use rand::{prelude::StdRng, Rng, SeedableRng};
use rayon::prelude::*;
use tokio::sync::oneshot;
use tracing::info;

pub trait SingleRoundClient<I: UInt>: Sync + Send {
    fn new<R: Rng>(input: &[I], rng: &mut R) -> Self;
    fn send_to_ot_sender(&self, id: SendId, conn: TcpConnection) -> oneshot::Receiver<()>;
    fn send_to_ot_receiver(&self, id: SendId, conn: TcpConnection) -> oneshot::Receiver<()>;
}

/// Client on input ring `I`, and correlation ring `C`
pub struct Po2Client<I: UInt> {
    pub prepared_message_0: ClientPo2MsgToAlice,
    pub prepared_message_1: ClientPo2MsgToBob<I>,
}

impl<I: UInt> SingleRoundClient<I> for Po2Client<I> {
    fn new<R: Rng>(input: &[I], rng: &mut R) -> Self {
        let gsize = input.len();
        let (input_0, input_1) = batch_make_boolean_shares(rng, input.iter().map(|x| x.bits_le()));
        let delta = COTGen::sample_delta(rng);
        let num_additional_cot = num_additional_ot_needed(gsize * I::NUM_BITS as usize);
        let (cot_s, cot_r) = COTGen::sample_cots(rng, &input_1, delta, num_additional_cot);

        let prepared_message_0 = ClientPo2MsgToAlice::new(input_0, cot_s);
        let prepared_message_1 = ClientPo2MsgToBob::new(input_1, cot_r);
        Po2Client {
            prepared_message_0,
            prepared_message_1,
        }
    }

    fn send_to_ot_sender(&self, id: SendId, conn: TcpConnection) -> oneshot::Receiver<()> {
        conn.send_message(id, &self.prepared_message_0).unwrap()
    }

    fn send_to_ot_receiver(&self, id: SendId, conn: TcpConnection) -> oneshot::Receiver<()> {
        conn.send_message(id, &self.prepared_message_1).unwrap()
    }
}

pub async fn start_one_round_client<I: UInt, C: SingleRoundClient<I>>(options: Options) {
    assert_eq!(options.input_size.num_bits(), I::NUM_BITS);
    tracing_subscriber::fmt()
        .pretty()
        .with_max_level(options.log_level)
        .init();

    info!(
        "num_clients: {}, Server address alice: {}, server address bob: {}, gsize: {}, log_level: {}",
        options.num_clients, options.server_alice, options.server_bob, options.gsize, options.log_level
    );

    let timer = start_timer!(|| "Preparing Client Input");
    let data = {
        (0..options.num_clients)
            .into_par_iter()
            .map(|i| {
                let mut rng = StdRng::seed_from_u64(i as u64);
                (0..options.gsize)
                    .map(|_| I::rand(&mut rng))
                    .collect::<Vec<I>>()
            })
            .collect::<Vec<Vec<I>>>()
    };
    end_timer!(timer);

    let mut rng = StdRng::from_entropy();
    let seeds = (0..options.num_clients)
        .map(|_| rng.gen::<u64>())
        .collect::<Vec<_>>();
    let timer = start_timer!(|| "Preparing Client Message");
    let clients = data
        .into_par_iter()
        .zip(seeds)
        .map(|(input, seed)| C::new(&input, &mut StdRng::seed_from_u64(seed)))
        .collect::<Vec<C>>();
    end_timer!(timer);

    info!("Attempting to connect to server");
    let connections = init_meta_clients(
        options.num_clients,
        &options.server_alice,
        &options.server_bob,
    )
    .await;

    info!("All clients connected! Sending clients data...");

    let handles = clients
        .into_par_iter()
        .zip(connections)
        .enumerate()
        .map(|(uid, (client, (conn_alice, conn_bob)))| {
            // alice is OT sender if uid is even
            let (ot_sender, ot_receiver) = if uid % 2 == 0 {
                (conn_alice, conn_bob)
            } else {
                (conn_bob, conn_alice)
            };
            assert_eq!(ot_sender.uid(), ot_receiver.uid());
            assert_eq!(ot_sender.uid(), (uid as u64).into());
            let h0 = client.send_to_ot_sender(SendId::FIRST, ot_sender);
            let h1 = client.send_to_ot_receiver(SendId::FIRST, ot_receiver);
            [h0, h1]
        })
        .flatten()
        .collect::<Vec<_>>();

    for h in handles {
        h.await.unwrap();
    }
}
