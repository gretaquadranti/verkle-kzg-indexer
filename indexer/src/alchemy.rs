use tokio_tungstenite::{connect_async, tungstenite::protocol::Message};
use futures_util::{StreamExt, SinkExt}; 
use serde_json::json;
use std::error::Error;
use std::pin::Pin;
use std::future::Future;
use reqwest::Client;
use crate::models::{JsonRPCRequest, JsonRPCResponse, Block};


// webSocket: mantiene una connessione persistente con Alchemy e riceve
// una notifica ogni volta che viene minato un nuovo blocco sulla blockchain
pub struct AlchemyWebSocket {
    url: String,
}

impl AlchemyWebSocket {

    pub fn new(api_key: String) -> Self {
        // uso wss x aprire un canale di comunicazione PERMANENTE 
        let url = format!("wss://eth-sepolia.g.alchemy.com/v2/{}", api_key);
        Self { url }
    }

    async fn connect_and_listen<F>(
    &self,
    callback: &mut F
) -> Result<(), Box<dyn Error + Send + Sync>>
where F: FnMut(String) -> Pin<Box<dyn Future<Output = ()> + Send>> + Send + 'static
{
    
    let (ws_stream, _) = connect_async(&self.url).await?; 
    println!("connected to webSocket");
    
    let (mut write, mut read) = ws_stream.split();

    //richiesta di iscrivermi al feed che annuncia quando ci sono nuovi blocchi
    let iscrizione = crate::models::JsonRPCRequest {
        jsonrpc: "2.0".to_string(),
        id: 1,
        method: "eth_subscribe".to_string(),
        params: vec![serde_json::json!("newHeads")], 
    };

    let iscrizione_str = serde_json::to_string(&iscrizione)?;
    write.send(Message::Text(iscrizione_str)).await?;
    
    //loop in ascolto.
    //read.next().await si sospende e aspetta il prossimo messaggio da Alchemy
    while let Some(mesg) = read.next().await {
        if mesg.is_err() {
            println!("network error: {:?}", mesg.err());
            continue; 
        }

        let messaggio = mesg.unwrap();
        if messaggio.is_text() {
            let testo = messaggio.to_string();
            
            let json_testo = serde_json::from_str::<serde_json::Value>(&testo);
            
            if json_testo.is_err() {
                continue;
            }

            let json_value = json_testo.unwrap();

            let params = json_value.get("params");
            if params.is_some() {
                
                let result = params.unwrap().get("result");
                if result.is_some() {
                    let number = result.unwrap().get("number");
                    if number.is_some() {
                        let numero_hex = number.unwrap().as_str().unwrap();
                        callback(numero_hex.to_string()).await; //chiamo la callback
                    }
                }
            }
        }

        // alchemy invia periodicamente un ping per verificare che la connessione
        // sia ancora attiva -> invio pong 
        else if messaggio.is_ping() {
            let dati = messaggio.into_data();
            write.send(Message::Pong(dati)).await?;
        }

        else if messaggio.is_close() {
            println!("server ha chiuso la connessione");
            break;
        }
    }
    
    println!("disconnected");
    Ok(())
}


    //punto di ingresso pubblico per avviare l'ascolto dei nuovi blocchi 
    pub async fn subscribe_new_blocks<F>(
        &self, 
        mut callback: F 
    ) -> Result<(), Box<dyn Error + Send + Sync>> where  
        F: FnMut(String) -> Pin<Box<dyn Future<Output = ()> + Send>> + Send + 'static
    {
        
        loop {
            println!("connecting to webSocket...");

            let res = self.connect_and_listen(&mut callback).await;
            
            match res {
                Ok(()) => { println!("webSocket connection closed gracefully. Reconnecting..."); }
                Err(e) => {
                    eprintln!("webSocket error: {}", e);
                    println!("reconnecting in 5 sec...");

                    //metto in pausa 5 sec prima di ritentare di nuovo di connettere
                    tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
                }
            }
        }
    }
}


//----------------------------------------------------------------------------------------------------------------
// per fare richieste "una tantum" --> serve per recuperare il passato
//durante il catch-up iniziale o quando la callback deve scaricare un blocco appena notificato dal WebSocket
pub struct AlchemyClient {
    http_client: Client,
    url: String,
}

impl AlchemyClient {
    pub fn new(api_key: String) -> Self {
        //richiesta HTTP
        let http_client = Client::new();
        let url = format!("https://eth-sepolia.g.alchemy.com/v2/{}", api_key);
        
        Self { http_client, url }
    }
    
   
    //chiede ad alchemy il numero dell'ultimo blocco minato sulla blockchain
    // metodo usato all'avvio per calcolare il gap tra l'ultimo blocco nel database
    // e il blocco piu recente della chain
    pub async fn get_latest_block_number(&self) -> Result<i64, Box<dyn Error + Send + Sync>> {
   
    //preparo la richiesta
        let request = JsonRPCRequest {
            jsonrpc: "2.0".to_string(),
            method: "eth_blockNumber".to_string(), 
            params: vec![],  
            id: 1,
        };
    
        let response = self.http_client
            .post(&self.url)
            .json(&request) 
            .send() 
            .await?;
    
        let result: JsonRPCResponse<String> = response.json().await?;
        
        let block_hex = result.result
            .ok_or("No result in response")?;
        
        let block_number = crate::utils::hex_to_i64(&block_hex)?;
        Ok(block_number)
    }
    
    
   //scarica il blocco completo dato il suo numero
    pub async fn get_block(&self, block_number: i64) -> Result<Block, Box<dyn Error + Send + Sync>> {
        let block_hex = format!("0x{:x}", block_number);
        
    
        let request = JsonRPCRequest {
            jsonrpc: "2.0".to_string(),
            method: "eth_getBlockByNumber".to_string(),
            params: vec![
                json!(block_hex),  
                json!(true)],
            id: 1,
        };
    
        let response = self.http_client
            .post(&self.url)
            .json(&request)
            .send()
            .await?;
        
        let response_text = response.text().await?;

        let result: JsonRPCResponse<Block> = serde_json::from_str(&response_text)
            .map_err(|e| {
                eprintln!("Failed to parse response for block {}: {}", block_number, e);
                eprintln!("Response body: {}", response_text);
                e
            })?;
    

        if let Some(error) = result.error {
            return Err(format!("RPC error: {:?}", error).into());
        }

        result.result.ok_or_else(|| "Block not found or null result".into())
    }
}
