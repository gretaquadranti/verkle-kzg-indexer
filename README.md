# Verkle Tree Indexer

Indexer per la blockchain Ethereum che scarica i blocchi, li salva su un database PostgreSQL e li inserisce in un **Verkle tree** i cui commitment sono calcolati con **KZG** (curva BLS12-381).

L'applicazione espone anche un'API HTTP che permette di richiedere una **prova di membership** per un qualsiasi blocco indicizzato e di verificarne la validità.

## Architettura

Il progetto è diviso in due crate Rust:

- **`verkle_kzg_impl`** – libreria che implementa lo schema di commitment KZG e il Verkle tree.
- **`indexer`** – applicazione principale che usa la libreria, parla con Alchemy, scrive sul DB.

### Avvio

1. **Chiave pubblica KZG.** Viene caricata dal file `trusted_setup.bin` (nella directory da cui si lancia il programma). Se il file non esiste ne viene generata una nuova e salvata.
2. **Ricostruzione dell'albero.** Si leggono dal DB tutti i blocchi già indicizzati e si reinseriscono nell'albero costruendo **solo la struttura**, senza calcolare nessun commitment.
3. **Caricamento dei commitment.** I commitment di tutti i nodi (tabella `verkle_nodes`) vengono letti dal DB e assegnati ai nodi tramite il loro percorso. Nessun commitment viene ricalcolato.
4. **Catch-up.** Si scaricano i blocchi mancanti rispetto alla chain. Ogni blocco aggiorna i commitment dei soli nodi sul suo percorso e viene salvato nel DB.
5. **Task in parallelo:**
   - **WebSocket** – resta in ascolto su Alchemy e indicizza ogni nuovo blocco in tempo reale.
   - **API HTTP** – serve le richieste sulla porta `3000`.

## Prerequisiti

- Rust
- PostgreSQL
- Account Alchemy con una API key

## Setup del database

Crea un database PostgreSQL e poi esegui le seguenti query per creare le tabelle necessarie:

CREATE TABLE blocks (
number BIGINT PRIMARY KEY,
hash VARCHAR(66) NOT NULL,
parent_hash VARCHAR(66) NOT NULL,
timestamp BIGINT NOT NULL,
miner VARCHAR(42) NOT NULL,
gas_used BIGINT NOT NULL,
gas_limit BIGINT NOT NULL,
transactions_count INTEGER NOT NULL,
size BIGINT NOT NULL
);

CREATE TABLE indexer_state (
id INTEGER PRIMARY KEY,
last_block_indexed BIGINT NOT NULL,
last_update TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

-- un nodo dell'albero per riga: percorso in esadecimale (2 caratteri per livello, radice = stringa vuota), tipo ("stem" o "branch") e commitment in 48 byte
CREATE TABLE verkle_nodes (
node_path TEXT PRIMARY KEY,
node_type TEXT NOT NULL,
commitment BYTEA NOT NULL
);

\*Inserisci il punto di partenza (modifica il numero con l'ultimo blocco già disponibile)

INSERT INTO indexer_state (id, last_block_indexed) VALUES (1, 0);

Il valore `0` in `indexer_state` significa che l'indexer partirà dall'inizio. Puoi impostare un numero più alto per saltare i blocchi più vecchi.

## Configurazione

Copia il file di esempio e compilalo con i tuoi dati:

cp env.example.txt .env

Contenuto del file `.env`:

ALCHEMY_API_KEY=la_tua_api_key
DB_USER=postgres
DB_PASSWORD=la_tua_password
DB_HOST=localhost
DB_NAME=nome_del_database

## Avvio

cd indexer
cargo run --release

> **Importante:** i commitment nel DB sono validi solo con la chiave pubblica con cui sono stati calcolati.
> Se cancelli o rigeneri `trusted_setup.bin` (o lanci il programma da un'altra cartella, che ne crea uno nuovo), svuota prima la tabella con `DELETE FROM verkle_nodes;` e ricalcola i commitment.

## API

### `GET /`

Pagina HTML di benvenuto.

### `GET /proof/:block_number`

Genera e verifica la prova di membership per il blocco con numero `block_number`.
La pagina HTML mostra:

- Il numero del blocco e il suo hash
- Il commitment della root del Verkle tree al momento della richiesta
- Il commitment nella prova
- Se la prova è valida o meno

## Come funziona il Verkle tree

Ogni blocco viene inserito nel Verkle tree usando:

- **Chiave**: il numero del blocco codificato in 32 byte. I byte del numero vengono copiati nelle posizioni 24..32, cioè negli ultimi 8 byte — quindi a destra. I primi 24 byte rimangono a zero.
- **Valore**: l'hash del blocco codificato in 48 byte

La chiave si divide in uno **stem** (i primi 31 byte, un byte per livello dell'albero) e un **suffisso** (l'ultimo byte, la posizione dentro lo stem). Ogni nodo ha 256 posizioni e un commitment KZG su di esse. In un branch ogni posizione contiene il commitment del figlio; in uno stem contiene il valore.

La prova di membership (`/proof/:block_number`) dimostra che un blocco è presente nel tree senza dover rivelare tutti gli altri blocchi.

## Ottimizzazioni

Il commitment di un nodo è calcolato come somma pesata dei suoi valori:

C = Σ valore_i · peso_i, dove peso_i = L_i(s) · G1

I pesi (`vettore_pesi`) dipendono solo dalle posizioni, non dai valori: sono calcolati **una volta sola** nel trusted setup e salvati nella chiave pubblica. Da questo derivano tre miglioramenti.

1. **Aggiornamento incrementale dei commitment.** Quando cambia una sola posizione di un nodo, il suo commitment si corregge con una moltiplicazione, senza interpolare e senza ricalcolare tutta la somma:

   `C_nuovo = C_vecchio + (valore_nuovo − valore_vecchio) · peso_posizione`

   Inserire un blocco costa quindi una moltiplicazione per ogni nodo del percorso (32 nodi: stem e 31 branch) e non un ricalcolo completo.

2. **Salvataggio dei commitment nel DB.** Al riavvio i commitment si ricaricano dalla tabella `verkle_nodes` invece di essere ricalcolati, riducendo il costo per nodo a una deserializzazione.

3. **Polinomi di base di Lagrange precalcolati per le prove.** Per costruire la prova serve il polinomio che passa per i 256 valori di un nodo. I 256 polinomi di base di Lagrange non dipendono dai valori, quindi vengono costruiti una sola volta (alla prima prova dopo l'avvio) e riutilizzati: la singola interpolazione passa da O(n³) a O(n²).
