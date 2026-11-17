use ark_bls12_381::{G1Affine, G1Projective};
use ark_serialize::CanonicalDeserialize;
use sha2::{Digest, Sha256};
use verkle_project::{VerkleTree, MembershipProof, VectorCommitment, kzg::load_pk};
use verkle_project::verkle_tree::ProofStep;


fn hex_to_coordinates(h: &str) -> G1Projective {
    //trasforma la stringa in esadecimale in 48 byte
    let b=hex::decode(h).expect("hex");
    G1Affine::deserialize_compressed(&b[..]).expect("punto non valido").into()
}

//per fare controllo su lunghezza chiave e valore
fn length_check<const N: usize>(h: &str) -> [u8; N] {
    hex::decode(h).expect("hex").try_into().expect("lunghezza errata")
}

fn main() {

    //leggo da console
    let a: Vec<String>= std::env::args().collect();
    let proof_j: serde_json::Value= serde_json::from_str(&std::fs::read_to_string(&a[1]).unwrap()).unwrap();
    let query_j: serde_json::Value= serde_json::from_str(&std::fs::read_to_string(&a[2]).unwrap()).unwrap();
    let root_trusted= hex_to_coordinates(&a[3]);   // radice da usare     

    //carico la chiave perchè serve per poter fare la verifica
    let pk= load_pk("trusted_setup.bin");

    //recupera il numero blocco che ho chiesto da proof json
    let n=proof_j["block_number"].as_i64().unwrap();
    // usando il numero del blocco poi si cerca all'interno di query per recuperare ulteriori campi

    let lista_blocchi=query_j["results"].as_array().unwrap();

    let mut trovato = None;
    for b in lista_blocchi {
        if b["block_number"].as_i64() == Some(n) {
            trovato = Some(b);
            break;
        }
    }
    let item = trovato.expect("blocco non nella query");

    let numero= item["block_number"].as_i64().unwrap();
    let hash= item["block_hash"].as_str().unwrap();
    let parent= item["parent_hash"].as_str().unwrap();
    let fee= item["fee_recipient"].as_str().unwrap();
    let timestamp= item["timestamp"].as_i64().unwrap();
    let gas_used= item["gas_used"].as_i64().unwrap();
    let gas_limit = item["gas_limit"].as_i64().unwrap();
    let tx_count= item["tx_count"].as_i64().unwrap();
    let size= item["size"].as_i64().unwrap();

    let s= format!(
        "BLOCK_V1|{}|{}|{}|{}|{}|{}|{}|{}|{}",
        numero, hash, parent, fee, timestamp, gas_used, gas_limit, tx_count, size
    );

    //devo creare hash tra questi valori cosi poi posso calcolare la verifica
    let digest =Sha256::digest(s.as_bytes());

    // controllo che il valore dentro la prova deve essere (digest || padding) padding serve perchè per ogni value la lunghezza deve essere di 48byte
    let value: [u8; 48]= length_check(proof_j["value"].as_str().unwrap());

    let mut atteso = [0u8; 48];
    atteso[..32].copy_from_slice(&digest);
    if value != atteso {
        println!("NOT VALID: il valore nella prova non coincide con i dati del blocco");
        return;
    }

    // dalla prova si prendono per ogni step: commitment value witness e indice 
    let lista_passi= proof_j["steps"].as_array().unwrap();
    let mut steps: Vec<ProofStep> = Vec::new();

    for passo in lista_passi {
        let commitment_hex=passo["commitment"].as_str().unwrap();
        let value_hex= passo["value"].as_str().unwrap();
        let witness_hex= passo["witness"].as_str().unwrap();
        let index = passo["index"].as_u64().unwrap() as usize;

        let nuovo_passo=ProofStep {
            commitment:VectorCommitment{inner:hex_to_coordinates(commitment_hex)},
            index,
            value:length_check(value_hex),
            witness:hex_to_coordinates(witness_hex),
        };
        steps.push(nuovo_passo);
    }


    let key: [u8; 32] = length_check(proof_j["key"].as_str().unwrap());
    let proof = MembershipProof {steps, value, key };

    let ok = VerkleTree::verify_proof(&proof, &pk, VectorCommitment { inner: root_trusted }, key);
    if ok {
        println!("VALID");
        println!("Verified block data:");
        println!("  block_number:  {}", numero);
        println!("  block_hash:    {}", hash);
        println!("  parent_hash:   {}", parent);
        println!("  fee_recipient: {}", fee);
        println!("  timestamp:     {}", timestamp);
        println!("  gas_used:      {}", gas_used);
        println!("  gas_limit:     {}", gas_limit);
        println!("  tx_count:      {}", tx_count);
        println!("  size:          {}", size);
    } else {
        println!("NOT VALID");
    }
}