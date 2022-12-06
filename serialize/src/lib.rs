pub mod util;

use crate::util::{ReadUtil, WriteUtil};
use bytemuck::Pod;
use bytes::{BufMut, Bytes, BytesMut};
use serde::{de::DeserializeOwned, Serialize};
use std::{
    any::Any,
    io::{Read, Write},
};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
    #[error("Error from serde: {0}")]
    SerdeError(#[from] bincode::Error),
    #[error("io Error: {0}")]
    IoError(#[from] std::io::Error),
    #[error("received malformed message: {0}")]
    ReceivedMalformedMessage(bytemuck::PodCastError),
}
pub type Result<T> = std::result::Result<T, Error>;

pub trait Communicate: Send + Sync {
    type Deserialized: Sized + Send + Sync + Any;
    fn size_in_bytes(&self) -> usize;
    /// convert `self` to `Bytes`
    fn to_bytes<W: Write>(&self, dest: W);
    /// optional specialization for constant time serialization by taking
    /// ownership of `self` to convert to `Bytes`
    fn into_bytes_owned(self) -> Bytes
    where
        Self: Sized,
    {
        let mut buf = BytesMut::with_capacity(self.size_in_bytes()).writer();
        self.to_bytes(&mut buf);
        buf.into_inner().freeze()
    }

    fn from_bytes<R: Read>(bytes: R) -> Result<Self::Deserialized>;

    /// optional specialization for constant time deserialization from `Bytes`
    fn from_bytes_owned(bytes: Bytes) -> Result<Self::Deserialized> {
        Self::from_bytes(bytes.as_ref())
    }
}

impl<T: Communicate> Communicate for &T {
    type Deserialized = T::Deserialized;

    fn size_in_bytes(&self) -> usize {
        (*self).size_in_bytes()
    }

    fn to_bytes<W: Write>(&self, dest: W) {
        (*self).to_bytes(dest)
    }

    fn from_bytes<R: Read>(bytes: R) -> Result<Self::Deserialized> {
        T::from_bytes(bytes)
    }
}

pub struct UseSerde<T: Serialize + DeserializeOwned + Send + Sync + Any>(pub T);

impl<T: Serialize + DeserializeOwned + Send + Sync + Any> Communicate for UseSerde<T> {
    type Deserialized = T;

    fn size_in_bytes(&self) -> usize {
        bincode::serialized_size(&self.0).unwrap() as usize
    }

    fn to_bytes<W: Write>(&self, dest: W) {
        bincode::serialize_into(dest, &self.0).unwrap();
    }

    fn from_bytes<R: Read>(bytes: R) -> Result<Self::Deserialized> {
        Ok(bincode::deserialize_from(bytes)?)
    }
}

pub struct UseCast<T: Pod + Send + Sync + Any>(pub T);

pub trait AsUseCast: Pod + Send + Sync + Any {
    fn use_cast(self) -> UseCast<Self>;
}

impl<T: Pod + Send + Sync + Any> AsUseCast for T {
    fn use_cast(self) -> UseCast<Self> {
        UseCast(self)
    }
}

impl<T: Pod + Send + Sync + Any> Communicate for UseCast<T> {
    type Deserialized = T;

    fn size_in_bytes(&self) -> usize {
        std::mem::size_of::<T>()
    }

    fn to_bytes<W: Write>(&self, mut dest: W) {
        dest.write_pod(&self.0).unwrap()
    }

    fn from_bytes<R: Read>(mut bytes: R) -> Result<Self::Deserialized> {
        Ok(bytes.read_pod()?)
    }
}

impl<T: Pod + Send + Sync + Any> Communicate for [T] {
    type Deserialized = Vec<T>;

    fn size_in_bytes(&self) -> usize {
        std::mem::size_of::<u64>() + std::mem::size_of::<T>() * self.len()
    }

    fn to_bytes<W: Write>(&self, mut dest: W) {
        let raw = bytemuck::cast_slice::<_, u8>(self);
        debug_assert_eq!(raw.len(), self.len() * std::mem::size_of::<T>());
        dest.write_pod(&(self.len() as u64)).unwrap();
        dest.write_all(raw).unwrap();
    }

    fn from_bytes<R: Read>(mut bytes: R) -> Result<Self::Deserialized> {
        let len = bytes.read_pod::<u64>()?;
        let result = (0..len)
            .map(|_| Ok(bytes.read_pod::<T>()?))
            .collect::<Result<Vec<T>>>()?;
        Ok(result)
    }
}

// impl<T: Communicate, const SIZE: usize> Communicate for [T; SIZE]
// where T::Deserialized: Default + Copy
// {
//     type Deserialized = [T::Deserialized; SIZE];
//
//     fn size_in_bytes(&self) -> usize {
//         self.iter().map(|x| x.size_in_bytes()).sum()
//     }
//
//     fn to_bytes<W: Write>(&self, mut dest: W) {
//         for x in self.iter() {
//             x.to_bytes(&mut dest);
//         }
//     }
//
//     fn from_bytes<R: Read>(mut bytes: R) -> Result<Self::Deserialized> {
//         let mut result = [T::Deserialized::default(); SIZE];
//         for x in result.iter_mut() {
//             *x = T::from_bytes(&mut bytes)?;
//         }
//         Ok(result)
//     }
// }

impl<T: Pod + Send + Sync + Any> Communicate for Vec<T> {
    type Deserialized = Vec<T>;

    fn size_in_bytes(&self) -> usize {
        self.as_slice().size_in_bytes()
    }

    fn to_bytes<W: Write>(&self, dest: W) {
        self.as_slice().to_bytes(dest)
    }

    fn from_bytes<R: Read>(bytes: R) -> Result<Self::Deserialized> {
        <[T] as Communicate>::from_bytes(bytes)
    }
}

macro_rules! impl_tuple{
    ($($i: tt, $ty: tt), +) => {
        impl<$($ty: Communicate), +> Communicate for ($($ty), +) {
            type Deserialized = ($($ty::Deserialized), +);

            #[inline]
            fn size_in_bytes(&self) -> usize {
                $(self.$i.size_in_bytes() +) + 0usize
            }

            #[inline]
            fn to_bytes<W: Write>(&self, mut dest: W) {
                $(self.$i.to_bytes(&mut dest)); +
            }

            #[inline]
            fn from_bytes<R: Read>(mut bytes: R) -> Result<Self::Deserialized> {
                Ok(($($ty::from_bytes(&mut bytes)?), +))
            }
        }
    };
}

impl_tuple!(0, T0, 1, T1);
impl_tuple!(0, T0, 1, T1, 2, T2);
impl_tuple!(0, T0, 1, T1, 2, T2, 3, T3);

impl Communicate for Bytes {
    type Deserialized = Bytes;

    fn size_in_bytes(&self) -> usize {
        self.len()
    }

    fn to_bytes<W: Write>(&self, mut dest: W) {
        dest.write_all(self).unwrap();
    }

    fn into_bytes_owned(self) -> Bytes
    where
        Self: Sized,
    {
        self
    }

    fn from_bytes<R: Read>(mut bytes: R) -> Result<Self::Deserialized> {
        let mut buf = Vec::new();
        bytes.read_to_end(&mut buf)?;
        Ok(buf.into())
    }

    fn from_bytes_owned(bytes: Bytes) -> Result<Self::Deserialized> {
        Ok(bytes)
    }
}

impl Communicate for () {
    type Deserialized = ();

    fn size_in_bytes(&self) -> usize {
        0
    }

    fn to_bytes<W: Write>(&self, dest: W) {
        let _ = dest;
    }

    fn from_bytes<R: Read>(bytes: R) -> Result<Self::Deserialized> {
        let _ = bytes;
        Ok(())
    }
}
