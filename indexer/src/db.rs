use sqlx::{PgPool, Transaction, Postgres, Row};
use crate::models::Block;
use crate::utils::hex_to_i64;
use std::error::Error;

// legge dal database l'ultimo numero di blocco che è stato indicizzato
pub async fn get_last_indexed_block(pool: &PgPool) -> Result<i64, Box<dyn Error + Send + Sync>> {
    let row = sqlx::query("SELECT last_block_indexed FROM indexer_state WHERE id = 1")
        .fetch_one(pool)
        .await?;
    
    let last_block: i64 = row.get(0);
    Ok(last_block)
}


// viene chiamato ogni volta che un blocco viene salvato con successo, in modo che
pub async fn update_last_indexed_block(
    pool: &PgPool, 
    block_number: i64
) -> Result<(), Box<dyn Error + Send + Sync>> {
    sqlx::query(
        "UPDATE indexer_state 
         SET last_block_indexed = $1, last_update = NOW() 
         WHERE id = 1"
    )
    .bind(block_number)
    .execute(pool)
    .await?;
    
    Ok(())
}


pub async fn save_block(
    db_transazione: &mut Transaction<'_, Postgres>,
    block: &Block
) -> Result<(), Box<dyn Error + Send + Sync>> {
    
    // i valori numerici arrivano come stringhe esadecimali e vengono convertiti in i64 prima di essere passati alla query
    let block_number = hex_to_i64(&block.number)?;
    let timestamp = hex_to_i64(&block.timestamp)?;
    let gas_used = hex_to_i64(&block.gas_used)?;
    let gas_limit = hex_to_i64(&block.gas_limit)?;
    let size = hex_to_i64(&block.size)?;
    let tx_count = block.transactions.len() as i32;
    
    sqlx::query(
        "INSERT INTO blocks 
         (number, hash, parent_hash, timestamp, miner, gas_used, gas_limit, transactions_count, size)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
         ON CONFLICT (number) DO NOTHING"
    )
    .bind(block_number)
    .bind(&block.hash)
    .bind(&block.parent_hash)
    .bind(timestamp)
    .bind(&block.miner)
    .bind(gas_used)
    .bind(gas_limit)
    .bind(tx_count)
    .bind(size)
    .execute(&mut **db_transazione)
    .await?;
    
    Ok(())
}


//al momento dell'avvio 
pub async fn get_all_blocks(pool: &PgPool)-> Result<Vec<(i64, String)>, Box<dyn Error + Send + Sync>> {
    
    let rows = sqlx::query("SELECT number, hash FROM blocks ORDER BY number ASC")
        .fetch_all(pool)
        .await?;
    
    let blocks = rows
        .iter()
        .map(|row| {
            let number: i64 = row.get(0);
            let hash: String = row.get(1);
            (number, hash)
        })
        .collect();
 
    Ok(blocks)
}



//  NUOVO salva o aggiorna il commitment di un nodo
pub async fn save_node(
    pool: &PgPool,
    path: &str,
    node_type: &str,
    commitment_bytes: &[u8],
) -> Result<(), Box<dyn Error + Send + Sync>> {
    sqlx::query(
        "INSERT INTO verkle_nodes (node_path, node_type, commitment)
         VALUES ($1, $2, $3)
         ON CONFLICT (node_path) DO UPDATE SET commitment = $3"
    )
    .bind(path)
    .bind(node_type)
    .bind(commitment_bytes)
    .execute(pool)
    .await?;
    Ok(())
}

// NUOVO carica tutti i nodi salvati
pub async fn load_all_nodes(
    pool: &PgPool,
) -> Result<Vec<(String, String, Vec<u8>)>, Box<dyn Error + Send + Sync>> {
    let rows = sqlx::query(
        "SELECT node_path, node_type, commitment FROM verkle_nodes"
    )
    .fetch_all(pool)
    .await?;

    let nodes = rows.iter().map(|row| {
        let path: String  = row.get(0);
        let ntype: String = row.get(1);
        let comm: Vec<u8> = row.get(2);
        (path, ntype, comm)
    }).collect();

    Ok(nodes)
}