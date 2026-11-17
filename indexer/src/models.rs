use serde::{Serialize, Deserialize};
use serde_json::Value;
use crate::utils::hex_to_i64;

#[derive(Serialize)]
pub struct JsonRPCRequest {
    pub jsonrpc:String,
    pub method:String,
    pub params:Vec<Value>,
    pub id:i32,
}

#[derive(Deserialize, Debug)]
pub struct JsonRPCResponse<T> {  
    pub id:i32,
    pub jsonrpc:String,
    pub result: Option<T>,  
    #[serde(default)]
    pub error: Option<Value>, 
}

//struttura per quando mi arriva come risposta un blocco
#[derive(Debug, Deserialize)]
pub struct Block {
    pub number:String,
    pub hash:String,
    #[serde(rename="parentHash")]
    pub parent_hash:String,
    pub timestamp:String,
    #[serde(rename="miner")]
    pub fee_recipient:String,
    #[serde(rename="gasUsed")]
    pub gas_used:String,
    #[serde(rename="gasLimit")]
    pub gas_limit:String,
    #[serde(default)]
    pub transactions:Vec<Value>,
    pub size: String,
}

//struttura per salvare il blocco
#[derive(Debug, Clone)]
pub struct BlockRecord {
    pub number:i64,
    pub hash:String,
    pub parent_hash:String,
    pub timestamp:i64,
    pub fee_recipient:String,
    pub gas_used:i64,
    pub gas_limit:i64,
    pub transactions_count:i32,
    pub size:i64,
}

impl BlockRecord {
    pub fn from_alchemy(b:&Block) -> Result<BlockRecord, Box<dyn std::error::Error+Send+Sync>> {
        Ok(BlockRecord {
            number:hex_to_i64(&b.number)?,
            hash:b.hash.clone(),
            parent_hash:b.parent_hash.clone(),
            timestamp:hex_to_i64(&b.timestamp)?,
            fee_recipient:b.fee_recipient.clone(),
            gas_used:hex_to_i64(&b.gas_used)?,
            gas_limit:hex_to_i64(&b.gas_limit)?,
            transactions_count:b.transactions.len() as i32,
            size:hex_to_i64(&b.size)?,
        })
    }
}







