use sha2::{Sha256, Digest};
use crate::models::BlockRecord;

//funzione per trasformare esadecimale in i64 
pub fn hex_to_i64(string_hex: &str) -> Result<i64, Box<dyn std::error::Error + Send + Sync>> {
    let no_prefix=string_hex.trim_start_matches("0x");
    Ok(i64::from_str_radix(no_prefix, 16)?)
}

// converte un numero di blocco in una chiave di 32 byte per il verkle tree
// i restanti 24 byte rimangono a zero
pub fn block_number_to_key(block_number: i64) -> [u8; 32] {
    let mut key=[0u8; 32];
    //to_be_bytes() converte l'intero a 64 bit in 8 byte in big endian
    let bytes=block_number.to_be_bytes(); 
    key[24..32].copy_from_slice(&bytes);
    
    key
}

// per costruire il digest a partire dal blockRecord
pub fn block_digest(b: &BlockRecord) -> [u8; 32] {
    let testo=format!(
        "BLOCK_V1|{}|{}|{}|{}|{}|{}|{}|{}|{}",
        b.number, b.hash, b.parent_hash, b.fee_recipient,
        b.timestamp, b.gas_used, b.gas_limit,
        b.transactions_count, b.size
    );
    Sha256::digest(testo.as_bytes()).into()
} 

//calcola il digest e lo trasforma in value a 48 byte
pub fn record_to_value(b: &BlockRecord) -> [u8; 48] {
    let mut v=[0u8; 48];
    v[..32].copy_from_slice(&block_digest(b));
    v
}