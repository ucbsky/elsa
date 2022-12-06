//! Messages used in protocols

/// Messages used in power of 2 protocol
pub mod po2 {
    use crate::{
        bits::{BitsLE, SeededInputShare},
        cot::{
            client::{num_additional_ot_needed, B2ACOTToAlice, B2ACOTToBob},
            ChoiceSeed,
        },
        uint::UInt,
    };
    use block::Block;
    use serialize::{AsUseCast, Communicate, UseCast};
    use std::io::{Read, Write};

    #[derive(Debug, Clone)]
    pub struct ClientPo2MsgToAlice {
        pub inputs_0: SeededInputShare,
        pub cot: B2ACOTToAlice, /* TODO: we need to add some extra OT for secure comparison simulation */
    }

    impl ClientPo2MsgToAlice {
        pub fn new(inputs_0_seed: SeededInputShare, cot: B2ACOTToAlice) -> Self {
            ClientPo2MsgToAlice {
                inputs_0: inputs_0_seed,
                cot,
            }
        }
    }

    impl Communicate for ClientPo2MsgToAlice {
        type Deserialized = Self;

        fn size_in_bytes(&self) -> usize {
            self.inputs_0.use_cast().size_in_bytes() + self.cot.size_in_bytes()
        }

        fn to_bytes<W: Write>(&self, mut dest: W) {
            self.inputs_0.use_cast().to_bytes(&mut dest);
            self.cot.to_bytes(&mut dest);
        }

        fn from_bytes<R: Read>(mut bytes: R) -> serialize::Result<Self::Deserialized> {
            let inputs_0 = UseCast::from_bytes(&mut bytes)?;
            let cot = B2ACOTToAlice::from_bytes(&mut bytes)?;
            Ok(ClientPo2MsgToAlice { inputs_0, cot })
        }
    }

    #[derive(Debug, Clone)]
    pub struct ClientPo2MsgToBob<T: UInt> {
        pub inputs_1: Vec<BitsLE<T>>,
        pub cot: B2ACOTToBob,
    }

    impl<T: UInt> ClientPo2MsgToBob<T> {
        pub fn new(inputs_1: Vec<BitsLE<T>>, cot: B2ACOTToBob) -> Self {
            ClientPo2MsgToBob { inputs_1, cot }
        }

        pub fn dummy(gsize: usize) -> Self {
            Self::dummy_with_custom_num_ots(
                gsize,
                gsize * T::NUM_BITS + num_additional_ot_needed(gsize * T::NUM_BITS),
            )
        }

        pub fn dummy_with_custom_num_ots(gsize: usize, num_ots: usize) -> Self {
            let inputs_1 = vec![BitsLE(T::zero()); gsize];
            let cot = B2ACOTToBob::new(
                ChoiceSeed(0),
                vec![Block::default(); num_ots + num_additional_ot_needed(num_ots)],
            );
            ClientPo2MsgToBob::new(inputs_1, cot)
        }
    }

    impl<T: UInt> Communicate for ClientPo2MsgToBob<T> {
        type Deserialized = Self;

        fn size_in_bytes(&self) -> usize {
            self.inputs_1.size_in_bytes() + self.cot.size_in_bytes()
        }

        fn to_bytes<W: Write>(&self, mut dest: W) {
            self.inputs_1.to_bytes(&mut dest);
            self.cot.to_bytes(&mut dest);
        }

        fn from_bytes<R: Read>(mut bytes: R) -> serialize::Result<Self::Deserialized> {
            let inputs_0 = Vec::from_bytes(&mut bytes)?;
            let cot = B2ACOTToBob::from_bytes(&mut bytes)?;
            Ok(ClientPo2MsgToBob {
                inputs_1: inputs_0,
                cot,
            })
        }
    }
}

pub mod l2 {
    use crate::{
        bits::{BitsLE, SeededInputShare},
        cot::client::{B2ACOTToAlice, B2ACOTToBob},
        malpriv::MessageHash,
        message::po2,
        square_corr::{CorrShareSeedToAlice, CorrShareSeedToBob},
        uint::UInt,
    };
    use serialize::Communicate;
    use std::io::{Read, Write};

    #[derive(Debug, Clone)]
    pub struct ClientL2MsgToAlice {
        pub po2_msg: po2::ClientPo2MsgToAlice,
        pub square_corr: CorrShareSeedToAlice,
    }

    impl ClientL2MsgToAlice {
        pub fn new(
            inputs_0_seed: SeededInputShare,
            cot: B2ACOTToAlice,
            square_corr: CorrShareSeedToAlice,
        ) -> Self {
            ClientL2MsgToAlice {
                po2_msg: po2::ClientPo2MsgToAlice::new(inputs_0_seed, cot),
                square_corr,
            }
        }

        #[inline]
        pub fn cot(&self) -> &B2ACOTToAlice {
            &self.po2_msg.cot
        }
    }

    impl Communicate for ClientL2MsgToAlice {
        type Deserialized = Self;

        fn size_in_bytes(&self) -> usize {
            self.po2_msg.size_in_bytes() + self.square_corr.size_in_bytes()
        }

        fn to_bytes<W: Write>(&self, mut dest: W) {
            self.po2_msg.to_bytes(&mut dest);
            self.square_corr.to_bytes(&mut dest);
        }

        fn from_bytes<R: Read>(mut bytes: R) -> serialize::Result<Self::Deserialized> {
            let po2_msg = po2::ClientPo2MsgToAlice::from_bytes(&mut bytes)?;
            let square_corr = CorrShareSeedToAlice::from_bytes(&mut bytes)?;
            Ok(ClientL2MsgToAlice {
                po2_msg,
                square_corr,
            })
        }
    }

    #[derive(Debug, Clone)]
    pub struct ClientL2MsgToBob<I: UInt, C: UInt> {
        pub po2_msg: po2::ClientPo2MsgToBob<I>,
        pub square_corr: CorrShareSeedToBob<C>,
    }

    impl<I: UInt, C: UInt> ClientL2MsgToBob<I, C> {
        pub fn new(
            inputs_1: Vec<BitsLE<I>>,
            cot: B2ACOTToBob,
            square_corr: CorrShareSeedToBob<C>,
        ) -> Self {
            ClientL2MsgToBob {
                po2_msg: po2::ClientPo2MsgToBob::new(inputs_1, cot),
                square_corr,
            }
        }

        #[inline]
        pub fn inputs_1(&self) -> &Vec<BitsLE<I>> {
            &self.po2_msg.inputs_1
        }

        #[inline]
        pub fn cot(&self) -> &B2ACOTToBob {
            &self.po2_msg.cot
        }
    }

    impl<I: UInt, C: UInt> Communicate for ClientL2MsgToBob<I, C> {
        type Deserialized = Self;

        fn size_in_bytes(&self) -> usize {
            self.po2_msg.size_in_bytes() + self.square_corr.size_in_bytes()
        }

        fn to_bytes<W: Write>(&self, mut dest: W) {
            self.po2_msg.to_bytes(&mut dest);
            self.square_corr.to_bytes(&mut dest);
        }

        fn from_bytes<R: Read>(mut bytes: R) -> serialize::Result<Self::Deserialized> {
            let po2_msg = po2::ClientPo2MsgToBob::from_bytes(&mut bytes)?;
            let square_corr = CorrShareSeedToBob::from_bytes(&mut bytes)?;
            Ok(ClientL2MsgToBob {
                po2_msg,
                square_corr,
            })
        }
    }

    pub type ClientMPMsgToAlice<H> = (
        (ClientL2MsgToAlice, <H as MessageHash>::Output),
        (<H as MessageHash>::Output, <H as MessageHash>::Output),
    );
    pub type ClientMPMsgToBob<I, C, H> = (
        (
            ClientL2MsgToBob<I, C>,
            <H as MessageHash>::Output,
            <H as MessageHash>::Output,
        ),
        <H as MessageHash>::Output,
    );
}
