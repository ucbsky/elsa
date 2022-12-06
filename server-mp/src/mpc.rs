use bridge::{id_tracker::ExchangeId, mpc_conn::MpcConnection};
use crypto_primitives::{
    a2s::{batch_a2s_first, batch_a2s_second},
    malpriv::MessageHash,
    square_corr::SquareCorrShare,
    uint::UInt,
    utils::SliceExt,
};
use rand::{rngs::StdRng, SeedableRng};

pub use server_mp_po2::mpc::*;

/// parties exchange their shares to open `d`. Return number of passed
/// correlations.
pub async fn corr_verify<C: UInt, const PARTY: bool, H: MessageHash>(
    msg_id1: ExchangeId,
    msg_id2: ExchangeId,
    input_len: usize,
    square_corr: &[SquareCorrShare<C>],
    t_seed: u64,
    peer: MpcConnection,
    hasher: &mut H,
) -> usize {
    let mut t_rng = StdRng::seed_from_u64(t_seed);

    assert_eq!(square_corr.len(), input_len * 2);
    let mut db = vec![C::zero(); input_len];
    let corr_b = &square_corr[..input_len];
    let sacr_b = &square_corr[input_len..];
    let t = (0..input_len)
        .map(|_| C::rand(&mut t_rng))
        .collect::<Vec<_>>();

    SquareCorrShare::verify_phase_1(corr_b, sacr_b, &t, &mut db);

    let db_other = if cfg!(feature = "no-comm") {
        vec![C::zero(); input_len]
    } else {
        peer.exchange_message(msg_id1, &db).await.unwrap()
    };

    // println!("db: {:x?}, db_other: {:x?}", db, db_other);

    hasher.absorb(&db_other);

    assert_eq!(db.len(), db_other.len());

    let d = db.zip_map(&db_other, |a, b| a.wrapping_add(b));

    let mut wb = vec![C::zero(); input_len];
    SquareCorrShare::verify_phase_2::<{ PARTY }>(&corr_b, &sacr_b, &t, &d, &mut wb);

    let wb_other = if cfg!(feature = "no-comm") {
        vec![C::zero(); input_len]
    } else {
        peer.exchange_message(msg_id2, &wb).await.unwrap()
    };

    hasher.absorb(&wb_other);

    assert_eq!(wb.len(), wb_other.len());

    wb.iter()
        .zip(wb_other.iter())
        .filter(|(a, b)| a.wrapping_add(b).is_zero())
        .count()
}

/// return the share of squares of each input
pub async fn a2s<A: UInt, C: UInt, H: MessageHash, const PARTY: bool>(
    msg_id: ExchangeId,
    xb: &[A],
    square_corr: &[SquareCorrShare<C>],
    peer: MpcConnection,
    hasher_other: &mut H,
) -> Vec<A> {
    let size = xb.len();
    let corr = square_corr[..size]
        .iter()
        .map(|x| x.cut())
        .collect::<Vec<SquareCorrShare<A>>>();
    assert_eq!(corr.len(), size);

    let eb = batch_a2s_first(xb, &corr);
    let eb_other = if cfg!(feature = "no-comm") {
        vec![A::zero(); size]
    } else {
        peer.exchange_message(msg_id, &eb).await.unwrap()
    };

    hasher_other.absorb(&eb_other);

    assert_eq!(eb.len(), eb_other.len());

    let e = eb.zip_map(&eb_other, |a, b| a.wrapping_add(b));

    let x_sq_b = batch_a2s_second::<_, PARTY>(&e, &xb, &corr);

    x_sq_b
    // secure comparison is ignored here, don't forget it in paper
}
