use super::setup::PublicKey;
use ark_bls12_381::{Fr, G1Projective};
use ark_ec::{VariableBaseMSM,CurveGroup};


// crea un commitment a un polinomio. restituisce quindi nel mio caso:
// - C = f(alpha)*G_1
// - w = q(alpha)*G_1
pub fn kzg_commit(coefficients: &[Fr], pk: &PublicKey) -> G1Projective {   

    let punti_projective = &pk.g1_vector[0..coefficients.len()];
    
    //metodo notmalize_batch che permette di trasformare da punti projective a punti affini
    let vec_punti_affine = G1Projective::normalize_batch(punti_projective);

    //la funzione msm prende i coef (di tipo Scalar), prende le g1*alpha (in base alle potenze) e crea il commitment 
    G1Projective::msm(&vec_punti_affine, coefficients).expect("MSM fallito")
}