use bytes::Bytes;
use crypto_primitives::{bits::BitsLE, uint::UInt};
use prio::{
    client::Client,
    encrypt::{PrivateKey, PublicKey},
    field::FieldElement,
};
use rand::Rng;

/// prepare data for one client instance
pub fn prepare_data<I: UInt, R: Rng>(gsize: usize, rng: &mut R) -> Vec<I> {
    (0..gsize).map(|_| I::rand(rng)).collect()
}

pub fn prepare_message<I: UInt, F: FieldElement>(data: &[I]) -> (Bytes, Bytes) {
    let priv_key2 = PrivateKey::from_base64(
        "BIl6j+J6dYttxALdjISDv6ZI4/VWVEhUzaS05LgrsfswmbLOgN\
         t9HUC2E0w+9RqZx3XMkdEHBHfNuCSMpOwofVSq3TfyKwn0NrftKisKKVSaTOt5seJ67P5QL4hxgPWvxw==",
    )
    .unwrap();
    let priv_key1 = PrivateKey::from_base64(
        "BNNOqoU54GPo+1gTPv+hCgA9U2ZCKd76yOMrWa1xTWgeb4LhF\
         LMQIQoRwDVaW64g/WTdcxT4rDULoycUNFB60LER6hPEHg/ObBnRPV1rwS3nj9Bj0tbjVPPyL9p8QW8B+w==",
    )
    .unwrap();

    let pub_key1 = PublicKey::from(&priv_key1);
    let pub_key2 = PublicKey::from(&priv_key2);

    // convert data to bits

    let data = data
        .iter()
        .map(|x| BitsLE(*x).iter())
        .flatten()
        .map(|x| if x { F::one() } else { F::zero() })
        .collect::<Vec<F>>();

    let dim = data.len();

    // Prio client object
    let mut prio_client = Client::new(dim, pub_key1, pub_key2).unwrap();

    // Encode the input along with SNIP proof
    let (data_share0, data_share1) = prio_client.encode_simple(&data).unwrap();

    (Bytes::from(data_share0), Bytes::from(data_share1))
}
