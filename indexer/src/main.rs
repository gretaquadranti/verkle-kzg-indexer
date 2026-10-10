mod models;
mod db;
mod alchemy;
mod utils;
mod api;
use dotenv::dotenv;
use std::env;
use sqlx::PgPool;
use std::pin::Pin;
use tokio::sync::Mutex;
use std::sync::Arc;
use verkle_project::{VerkleTree, kzg::save_or_load_pk};


// viene avviato un thread dedicato con uno stack allargato (64 MB invece
// dei 2 MB predefiniti) perché sennò stack overflow
fn main() {
    let stack_size = 64 * 1024 * 1024; 
    let builder = std::thread::Builder::new().stack_size(stack_size);
    
    let handler = builder.spawn(|| {
        tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(async_operation()) 
            .unwrap();
    }).unwrap();

    handler.join().unwrap();
}





async fn async_operation() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    dotenv().ok();

    // -------------------------DB CONNESSIONE--------------------------------------
    let db_conn = format!(
        "postgres://{}:{}@{}/{}",
        env::var("DB_USER")?,
        env::var("DB_PASSWORD")?,
        env::var("DB_HOST")?,
        env::var("DB_NAME")? 
    );
    
    let db_pool = PgPool::connect(&db_conn).await?;
    println!("database connected");
    let db_pool = Arc::new(db_pool);

    // ----------------------SETUP DEL VERKLE-----------------------------------------

    let pk = save_or_load_pk(255);
    //l'albero può essere condiviso + mutex 
    let tree = Arc::new(Mutex::new(VerkleTree::new(pk)));
    println!("verkle tree initialized");

    // -----------------------ALCHEMY SETUP----------------------------------------

    let api_key = env::var("ALCHEMY_API_KEY").expect("ALCHEMY_API_KEY mancante");
    let alchemy_http = Arc::new(alchemy::AlchemyClient::new(api_key.clone()));

    // --------------------------FASE DI CATCH UP-------------------------------------
    
    // all'avvio confronta l'ultimo blocco registrato nel db con
    // l'ultimo blocco disponibile sulla blockchain
    //se esiste un gap, i blocchi mancanti vengono scaricati, salvati sul DB e inseriti nel Verkle tree prima che il listener
    let last_indexed = db::get_last_indexed_block(&db_pool).await?; 
    let latest_on_chain = alchemy_http.get_latest_block_number().await?;
    println!("last indexed: {}", last_indexed);
    println!("latest on chain: {}", latest_on_chain);


    let mut tree_lock = tree.lock().await;

    //recupero tutti i blocchi dal db precedentemente salvati
    let blocks = db::get_all_blocks(&db_pool).await?;
    for (numero, hash) in &blocks {
        let key   = utils::block_number_to_key(*numero);
        let value = utils::hash_to_value(hash);

        tree_lock.insert(key, value,false); //FALSE perche non ricalcola commitment
    }

    // ora che ho ricreato il verkle tree, ci posso pure aggiungere ai vari nodi i commitment
    // carica i commitment esistenti dal db e li associa al nodo
    let nodes = db::load_all_nodes(&db_pool).await?;
    tree_lock.load_commitments_from_db(nodes);





        
    let gap = latest_on_chain - last_indexed;
    
    if gap > 0 {
        println!("gap detected: {} blocks", gap);

        for block_num in (last_indexed + 1)..=latest_on_chain {
            let mut tentativi=0;           
            println!("elaborazione blocco {}", block_num);

            while tentativi <3{
                match alchemy_http.get_block(block_num).await {
                    Ok(block) => {
                        let key   = utils::block_number_to_key(block_num);
                        let value = utils::hash_to_value(&block.hash);
                        
                        let mods = tree_lock.insert(key, value, true);
                        let nodes: Vec<(String, String, Vec<u8>)> = mods
                        .into_iter()
                        .map(|m| (m.path, m.node_type, m.commitment_bytes.to_vec()))
                        .collect();

                        db::persist_block(&db_pool, &block, block_num, &nodes).await?;
                        break;
                    }
                    Err(e) => {
                        tentativi+=1;
                        eprintln!("tentativo {} fallito per il blocco {}: {}, riprovo...", tentativi, block_num, e);
                        
                        if tentativi >= 3 { //se per tre volte non risolto problema. si blocca i programmma
                            return Err(
                            format!(
                            "catch-up interrotto: blocco {} irraggiungibile dopo 3 tentativi: {}",
                                block_num, e
                            ).into());
                        
                        }
                        tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;

                    }      
                }
            }
        }                   
    }    

    println!("scaricamento completato");

    drop(tree_lock);
    println!("catch-up completo");
    
    let tree_lock: tokio::sync::MutexGuard<'_, VerkleTree> = tree.lock().await;
    if let Some(root) = tree_lock.get_root_commitment() {
        println!("RADICE DEL TREE: {:?}", root);
    }



    drop(tree_lock);

       
    // ----------------------------SETUP PER TASK-----------------------------------
    
    let tree_ws    = Arc::clone(&tree);
    let tree_api   = Arc::clone(&tree);
    let alchemy_clone = Arc::clone(&alchemy_http);
    let db_clone      = Arc::clone(&db_pool); //pgPool è thread safe

    // -------------------------TASK 1: WEBSOCKET--------------------------------------
    
    
    let ws_task = tokio::spawn(async move {
        let ws = alchemy::AlchemyWebSocket::new(api_key);

        //riceve blocco hex
        let callback = move |block_hex: String| -> Pin<Box<dyn std::future::Future<Output = ()> + Send + 'static>> {
            
            let alchemy = Arc::clone(&alchemy_clone);
            let db      = Arc::clone(&db_clone);
            let tree    = Arc::clone(&tree_ws);

            Box::pin(async move { 
                let Ok(block_num) = utils::hex_to_i64(&block_hex) else {
                    eprintln!("error parsing block number: {}", block_hex);
                    return;
                };
                eprintln!("new block: {}", block_num);

                let Ok(ultimo_blocco_letto) = db::get_last_indexed_block(&db).await else {
                    eprintln!("error getting last indexed block");
                    return;
                };

                if block_num <= ultimo_blocco_letto {
                    eprintln!("block {} already indexed", block_num);
                    return;
                }


                let gap_ws = block_num - ultimo_blocco_letto;
                if gap_ws > 1 {
                    println!("gap rilevato nel websocket: {} blocchi mancanti ({} → {})", gap_ws, ultimo_blocco_letto, block_num);
                }

                for num in (ultimo_blocco_letto + 1)..=block_num {
                    println!("ws: elaborazione blocco {}", num);
                       
                    let block = match alchemy.get_block(num).await {
                        Ok(b) => b,
                        Err(e) => { 
                            eprintln!("error indexing block {}: {}", num, e); 
                            break;
                        }
                    };

                    let key   = utils::block_number_to_key(num);
                    let value = utils::hash_to_value(&block.hash);
                    
                    let mut tree_lock = tree.lock().await;
                    let ultimo_aggiornato = match db::get_last_indexed_block(&db).await{
                        Ok(v) => v,
                        Err(e) => { eprintln!("error reading last indexed: {}", e); continue; }
                    };

                    if num <= ultimo_aggiornato {
                        eprintln!("block {} già inserito da altro callback, skip", num);
                        continue;
                    }

                    let start = std::time::Instant::now();

                    let mods = tree_lock.insert(key, value, true);
                    let nodes: Vec<(String, String, Vec<u8>)> = mods
                        .into_iter()
                        .map(|m| (m.path, m.node_type, m.commitment_bytes.to_vec()))
                        .collect();

                    if let Err(e) = db::persist_block(&db, &block, num, &nodes).await {
                        eprintln!("ERRORE persistenza blocco {}: {}. Mi fermo per non desincronizzare.", num, e);
                        std::process::exit(1);
                    }
                    drop(tree_lock);
                    println!("block {} indexed e inserito nel tree", num);
                    println!("inserimento web_socket: {:?}", start.elapsed());

                }
            })
        };                
        _ = ws.subscribe_new_blocks(callback).await;   
    });

    // -----------------------TASK 2: API HTTP---------------------------------------
   
    let api_task = tokio::spawn(async move {

        let router = api::build_router(tree_api);

        //apre la porta 
        let listener = tokio::net::TcpListener::bind("127.0.0.1:3000")
            .await
            .expect("impossibile aprire porta 3000");

        
        println!("API server in ascolto su http://localhost:3000");
        axum::serve(listener, router)
            .await
            .expect("errore server HTTP");
    });
    let _ = tokio::join!(ws_task, api_task);

    Ok(())

}    

