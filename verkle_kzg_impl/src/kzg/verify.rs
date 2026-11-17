use super::setup::PublicKey;
use ark_bls12_381::{Fr, G1Projective, Bls12_381};
use ark_ec::pairing::Pairing;



// e(C, G2) == e(w, alpha·G2 - x0·G2) · e(y·G1, G2)
pub fn kzg_verify(
    commitment: G1Projective,
    index: usize,
    y: Fr,
    witness: G1Projective,
    pk: &PublicKey,
) -> bool {
    
    let x0 = Fr::from(index as u64);

    let g1: G1Projective = pk.g1_vector[0]; //sarebbe alpha^0* G1

    let g2 = pk.g2_vector[0];
    let alpha_g2 = pk.g2_vector[1]; // questo rappresenta alpha·G2 
    
    //calcolo alpha·g2 - x0·g2
    let s_minus_x0_g2 = alpha_g2 - (g2 * x0);
    
    let y_g1 = g1 * y;

    //pairing:
    //LATO SINISTRO: e(C, g2)
    let lhs = Bls12_381::pairing(commitment, g2);
    
    //LATO DESTRO: e(w, s_minus_x0_g2) · e(y_g1, g2)
    let rhs_1 = Bls12_381::pairing(witness,s_minus_x0_g2);
    let rhs_2 = Bls12_381::pairing(y_g1, g2);
    let rhs = rhs_1 + rhs_2;  
    
    lhs == rhs
}


