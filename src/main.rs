use dusk_bls12_381::{BlsScalar as Scalar, G1Projective, G2Projective};
use group::GroupEncoding;
use primitive_types::{H384, H768};

fn get_random_scalar() -> Scalar {
    let mut bytes = [0u8; 64];
    getrandom::fill(&mut bytes).unwrap();
    Scalar::from_bytes_wide(&bytes)
}

fn main() {
    const N: usize = u32::MAX as usize;
    let tau = get_random_scalar();
    let mut g1 = G1Projective::generator();
    let mut g2 = G2Projective::generator();
    for i in 0..N {
        g1 *= tau;
        g2 *= tau;
        let hex1 = H384::from_slice(g1.to_bytes().as_ref());
        let hex2 = H768::from_slice(g2.to_bytes().as_ref());
        println!("{}: {:#x} {:#x}", i, hex1, hex2);
    }
}
