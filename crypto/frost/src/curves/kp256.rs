use core::{marker::PhantomData, convert::TryInto};

use rand_core::{RngCore, CryptoRng};

use ff::{Field, PrimeField};
use group::{Group, GroupEncoding};

use sha2::{digest::Update, Digest, Sha256};

#[cfg(feature = "k256")]
use k256::elliptic_curve::bigint::{Encoding, U384};
#[cfg(all(not(feature = "k256"), any(test, feature = "p256")))]
use p256::elliptic_curve::bigint::{Encoding, U384};

use crate::{CurveError, Curve, curves::expand_message_xmd_sha256};

#[allow(non_snake_case)]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct KP256<P: Group> {
  _P: PhantomData<P>
}

pub(crate) trait KP256Instance<P> {
  const CONTEXT: &'static [u8];
  const ID: &'static [u8];
  const GENERATOR: P;
}

#[cfg(any(test, feature = "p256"))]
pub type P256 = KP256<p256::ProjectivePoint>;
#[cfg(any(test, feature = "p256"))]
impl KP256Instance<p256::ProjectivePoint> for P256 {
  const CONTEXT: &'static [u8] = b"FROST-P256-SHA256-v5";
  const ID: &'static [u8] = b"P-256";
  const GENERATOR: p256::ProjectivePoint = p256::ProjectivePoint::GENERATOR;
}

#[cfg(feature = "k256")]
pub type K256 = KP256<k256::ProjectivePoint>;
#[cfg(feature = "k256")]
impl KP256Instance<k256::ProjectivePoint> for K256 {
  const CONTEXT: &'static [u8] = b"FROST-secp256k1-SHA256-v5";
  const ID: &'static [u8] = b"secp256k1";
  const GENERATOR: k256::ProjectivePoint = k256::ProjectivePoint::GENERATOR;
}

impl<P: Group + GroupEncoding> Curve for KP256<P> where
  KP256<P>: KP256Instance<P>,
  P::Scalar: PrimeField,
  <P::Scalar as PrimeField>::Repr: From<[u8; 32]> + AsRef<[u8]>,
  P::Repr: From<[u8; 33]> + AsRef<[u8]> {
  type F = P::Scalar;
  type G = P;
  type T = P;

  const ID: &'static [u8] = <Self as KP256Instance<P>>::ID;

  const GENERATOR: Self::G = <Self as KP256Instance<P>>::GENERATOR;
  const GENERATOR_TABLE: Self::G = <Self as KP256Instance<P>>::GENERATOR;

  const LITTLE_ENDIAN: bool = false;

  fn random_nonce<R: RngCore + CryptoRng>(secret: Self::F, rng: &mut R) -> Self::F {
    let mut seed = vec![0; 32];
    rng.fill_bytes(&mut seed);
    seed.extend(secret.to_repr().as_ref());
    Self::hash_to_F(&[Self::CONTEXT, b"nonce"].concat(), &seed)
  }

  fn hash_msg(msg: &[u8]) -> Vec<u8> {
    (&Sha256::new()
      .chain(Self::CONTEXT)
      .chain(b"digest")
      .chain(msg)
      .finalize()
    ).to_vec()
  }

  fn hash_binding_factor(binding: &[u8]) -> Self::F {
    Self::hash_to_F(&[Self::CONTEXT, b"rho"].concat(), binding)
  }

  fn hash_to_F(dst: &[u8], msg: &[u8]) -> Self::F {
    let mut modulus = vec![0; 16];
    modulus.extend((Self::F::zero() - Self::F::one()).to_repr().as_ref());
    let modulus = U384::from_be_slice(&modulus).wrapping_add(&U384::ONE);
    Self::F_from_slice(
      &U384::from_be_slice(
        &expand_message_xmd_sha256(dst, msg, 48).unwrap()
      ).reduce(&modulus).unwrap().to_be_bytes()[16 ..]
    ).unwrap()
  }

  fn F_len() -> usize {
    32
  }

  fn G_len() -> usize {
    33
  }

  fn F_from_slice(slice: &[u8]) -> Result<Self::F, CurveError> {
    let bytes: [u8; 32] = slice.try_into()
      .map_err(|_| CurveError::InvalidLength(32, slice.len()))?;

    let scalar = Self::F::from_repr(bytes.into());
    if scalar.is_none().into() {
      Err(CurveError::InvalidScalar)?;
    }

    Ok(scalar.unwrap())
  }

  fn G_from_slice(slice: &[u8]) -> Result<Self::G, CurveError> {
    let bytes: [u8; 33] = slice.try_into()
      .map_err(|_| CurveError::InvalidLength(33, slice.len()))?;

    let point = Self::G::from_bytes(&bytes.into());
    if point.is_none().into() || point.unwrap().is_identity().into() {
      Err(CurveError::InvalidPoint)?;
    }

    Ok(point.unwrap())
  }

  fn F_to_bytes(f: &Self::F) -> Vec<u8> {
    f.to_repr().as_ref().to_vec()
  }

  fn G_to_bytes(g: &Self::G) -> Vec<u8> {
    g.to_bytes().as_ref().to_vec()
  }
}
