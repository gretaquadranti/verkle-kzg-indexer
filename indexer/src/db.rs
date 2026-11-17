use sqlx::{PgPool, Transaction, Postgres, Row};
use sqlx::postgres::PgRow;
use std::error::Error;
use crate::models::BlockRecord;

// trasforma una riga letta dal database in un BlockRecord
fn row_to_block(row:&PgRow)-> BlockRecord {
    BlockRecord {
        number:row.get("number"),
        hash:row.get("hash"),
        parent_hash:row.get("parent_hash"),
        timestamp:row.get("timestamp"),
        fee_recipient:row.get("fee_recipient"),
        gas_used:row.get("gas_used"),
        gas_limit: row.get("gas_limit"),
        transactions_count: row.get("transactions_count"),
        size:row.get("size"),
    }
}

// legge dal database l'ultimo numero di blocco che è stato indicizzato
pub async fn get_last_indexed_block(pool:&PgPool) -> Result<i64, Box<dyn Error + Send + Sync>> {
    let row=sqlx::query("SELECT last_block_indexed FROM indexer_state WHERE id = 1")
        .fetch_one(pool)
        .await?;
    
    let last_block: i64=row.get(0);
    Ok(last_block)
}

pub async fn save_block(tx: &mut Transaction<'_, Postgres>, b: &BlockRecord) -> Result<(), Box<dyn Error + Send + Sync>> {
    sqlx::query(
        "INSERT INTO blocks (number, hash, parent_hash, timestamp, fee_recipient, gas_used, gas_limit, transactions_count, size)
         VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9) ON CONFLICT (number) DO NOTHING")
        .bind(b.number).bind(&b.hash).bind(&b.parent_hash).bind(b.timestamp)
        .bind(&b.fee_recipient).bind(b.gas_used).bind(b.gas_limit)
        .bind(b.transactions_count).bind(b.size)
        .execute(&mut **tx).await?;
    Ok(())
}

//al momento dell'avvio 
pub async fn get_all_blocks(pool:&PgPool) -> Result<Vec<BlockRecord>, Box<dyn Error + Send + Sync>> {
    let righe= sqlx::query(
        "SELECT number, hash, parent_hash, timestamp, fee_recipient, gas_used, gas_limit, transactions_count, size FROM blocks ORDER BY number ASC")
        .fetch_all(pool)
        .await?;

    let mut blocchi= Vec::new();
    for riga in righe {
        blocchi.push(row_to_block(&riga));
    }
    Ok(blocchi)
}

pub async fn load_all_nodes(pool: &PgPool,) -> Result<Vec<(String, String, Vec<u8>)>, Box<dyn Error + Send + Sync>> {
    let rows = sqlx::query(
        "SELECT node_path, node_type, commitment FROM verkle_nodes"
    )
    .fetch_all(pool)
    .await?;

    let nodes = rows.iter().map(|row| {
        let path: String=row.get(0);
        let ntype: String=row.get(1);
        let comm: Vec<u8> =row.get(2);
        (path, ntype, comm)
    }).collect();

    Ok(nodes)
}


pub async fn persist_block( pool: &PgPool,rec: &BlockRecord,nodes: &[(String, String, Vec<u8>)],   // (path, tipo, commitment)
) -> Result<(), Box<dyn Error + Send + Sync>> {
    let mut tx = pool.begin().await?;

    save_block(&mut tx, rec).await?;

    for (path, ntype, comm) in nodes {
        sqlx::query(
            "INSERT INTO verkle_nodes (node_path, node_type, commitment)
             VALUES ($1, $2, $3)
             ON CONFLICT (node_path) DO UPDATE SET commitment = $3")
            .bind(path)
            .bind(ntype)
            .bind(comm)
            .execute(&mut *tx)
            .await?;
    }

    sqlx::query("UPDATE indexer_state SET last_block_indexed = $1, last_update = NOW() WHERE id = 1")
        .bind(rec.number)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(())
}

pub async fn query_blocks( pool:&PgPool,fee_recipient:Option<&str>,from_ts:Option<i64>,to_ts:Option<i64>, min_tx: Option<i32>,limit:i64) -> Result<Vec<BlockRecord>, Box<dyn Error + Send + Sync>> {
    let righe = sqlx::query(
        "SELECT number, hash, parent_hash, timestamp, fee_recipient, gas_used, gas_limit, transactions_count, size
         FROM blocks
         WHERE ($1::text IS NULL OR fee_recipient= $1)
           AND ($2::bigint IS NULL OR timestamp>= $2)
           AND ($3::bigint IS NULL OR timestamp<= $3)
           AND ($4::int IS NULL OR transactions_count>=$4)
         ORDER BY number DESC
         LIMIT $5")
        .bind(fee_recipient)
        .bind(from_ts)
        .bind(to_ts)
        .bind(min_tx)
        .bind(limit)
        .fetch_all(pool)
        .await?;

    let mut blocchi = Vec::new();
    for riga in righe {
        blocchi.push(row_to_block(&riga));
    }
    Ok(blocchi)
}
