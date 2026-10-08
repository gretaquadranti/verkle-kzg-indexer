//funzione per trasformare esadecimale in i64 D
pub fn hex_to_i64(string_hex: &str) -> Result<i64, Box<dyn std::error::Error + Send + Sync>> {
    let no_prefix = string_hex.trim_start_matches("0x");

    Ok(i64::from_str_radix(no_prefix, 16)?)
}

// converte un numero di blocco in una chiave di 32 byte per il verkle tree
// i restanti 24 byte rimangono a zero
pub fn block_number_to_key(block_number: i64) -> [u8; 32] {
    let mut key = [0u8; 32];

    //to_be_bytes() converte l'intero a 64 bit in 8 byte in big endian
    let bytes = block_number.to_be_bytes(); 

    // copia quei 8 byte nei primi 8 posti dell'array 
    key[24..32].copy_from_slice(&bytes);
    
    key
}


pub fn hash_to_value(v: &str) -> [u8; 48] {

    let mut value = [0u8; 48];

    let no_prefix = v.trim_start_matches("0x");
    
    //hex::decode metodo che converte la stringa esadecimale in byte
    let bytes = hex::decode(no_prefix).unwrap_or_default();

    let len = bytes.len();
    value[0..len].copy_from_slice(&bytes[0..len]);
    
    value
}