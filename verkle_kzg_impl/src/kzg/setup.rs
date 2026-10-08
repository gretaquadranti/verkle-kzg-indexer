use ark_bls12_381::{Fr, G1Projective, G2Projective};
use ark_ec::Group;
use rand::thread_rng;
use ark_ff::{UniformRand, Field};
use ark_serialize::{CanonicalSerialize, CanonicalDeserialize};   // <-- NUOVO
use std::fs;      

#[derive(Clone, CanonicalSerialize, CanonicalDeserialize)]  
    pub struct PublicKey {
    pub t: usize,
     // [g1, alpha·g1, α^2·g1, ..., α^max_degree·g1]
    pub g1_vector: Vec<G1Projective>,
    // [g2, alpha·g2]
    pub g2_vector: Vec<G2Projective>,
    // polinomio base * pk
    pub vettore_pesi: Vec<G1Projective> 
}

pub fn trusted_setup(t: usize) -> PublicKey  {
        //genero scalar è un numero nel campo finito F_p
        let mut random_number_generator = thread_rng(); 
        let s = Fr::rand(&mut random_number_generator);
        
        let g1 = G1Projective::generator();
        
        let g2 = G2Projective::generator();
        
        //creo un vettore della grandezza del massimo degree +1, perchè c'è da contare s^0 
        let mut g1_vector = Vec::with_capacity(t + 1);
        
        let mut current_pw = Fr::from(1u64);
        
        //inserisco ogni valore g1*valore segreto 
        for _ in 0..=t {
            g1_vector.push(g1 * current_pw);
            current_pw = current_pw * s;
        }
        
        let g2_vector = vec![
            g2,        
            (g2 * s)   
        ];

        //s non viene piu usato, quindi "distrutto"

        let vettore_pesi= creazione_vettore_pesi(t,s,g1);

        PublicKey {
        t, 
        g1_vector,
        g2_vector,
        vettore_pesi
    }
        
    }

fn creazione_vettore_pesi(t: usize, s: Fr, g1: G1Projective) -> Vec<G1Projective>{

    let n = t + 1; // 256 posizioni
    let mut pesi = Vec::with_capacity(n);

    for i in 0..n {
        let mut num = Fr::from(1u64);
        let mut den = Fr::from(1u64);
        for j in 0..n {
            if j != i {
                num *= s - Fr::from(j as u64);
                den *= Fr::from(i as u64) - Fr::from(j as u64);
            }
        }
        pesi.push(g1 * (num * den.inverse().unwrap()));
    }

    pesi  
}


const PK_FILE: &str = "trusted_setup.bin";

pub fn save_or_load_pk(t: usize) -> PublicKey {
    if let Ok(bytes) = fs::read(PK_FILE) {
        println!("chiave pubblica trovata");
        PublicKey::deserialize_compressed(&bytes[..]).expect("file della chiave pubblica corrotto")
    } else {
        println!("nessuna chiave pubblica trovata, ne genero una nuova");
        let pk = trusted_setup(t);
        let mut bytes = Vec::new();
        pk.serialize_compressed(&mut bytes).expect("errore serializzazione pk");
        fs::write(PK_FILE, bytes).expect("impossibile scrivere il file della chiave pubblica");
        pk
    }
}