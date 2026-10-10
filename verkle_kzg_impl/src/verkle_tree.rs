use crate::kzg::PublicKey;
use crate::vector_commitment::{VectorCommitment, update_commitment,  commit_vector, prove_element, verify_element,  commitment_to_value};
use ark_bls12_381::G1Projective;
use ark_serialize::CanonicalDeserialize;
use ark_bls12_381::G1Affine;
//la chiave è un vettore da 32 byte
pub type Key = [u8; 32];     
pub type Value = [u8; 48];  
pub type Stem = [u8; 31];    //primi 31 byte della chiave
pub type Suffix = u8;        //ultimo byte 


#[derive(Debug, Clone)]
//nodi interni, ogni nodo puo avere al max 256 figli
pub struct BranchNode {
    //array dei 256 figli
    pub children: [Option<NodeRef>; 256],
    //commitment al commitment dei figli
    pub commitment: Option<VectorCommitment>,
}


#[derive(Debug, Clone)]
//stem node, ovvero 31 byte uguali, conterrà al max 256 valori che condividono i primi 31 byte
pub struct StemNode {
    pub stem: Stem,
    pub values: [Option<Value>; 256],
    pub commitment: Option<VectorCommitment>,
}

#[derive(Debug, Clone)]
pub enum NodeRef {
    Branch(Box<BranchNode>),
    Stem(Box<StemNode>),
}

//------------------------------------------------------------
// struttura per i nodi da salvare
//struttura per salvare i percorsi + commitment modificati
pub struct ModifyNode {
    pub path: String,
    pub node_type: String,  
    pub commitment_bytes: [u8; 48],
}

//------------------------------------------------------------
//

// metodi per le chiavi
pub fn get_stem(key: &Key) -> Stem {
    let mut stem = [0u8; 31];
    stem.copy_from_slice(&key[0..31]);
    stem
}

pub fn get_suffix(key: &Key) -> Suffix {
    key[31]
}

//------------------------------------------------------------
//BRANCH E STEM IMPL
impl BranchNode {
    pub fn new() -> Self {
        BranchNode {
            children: [const { None }; 256],
            commitment: None,  
        }
    }
    
    // per calcolare il commitment del branchNode
    pub fn compute_commitment(&mut self, pk: &PublicKey)-> VectorCommitment {    
        
        let mut child_values = [[0u8; 48]; 256];
        
        for i in 0..256 {
            match &self.children[i] {
                None => {
                    //lascio lo zero che c'è dall'iniizializzazione
                }
                
                 Some(NodeRef::Stem(stem_node)) => {
                    if let Some(commitment) = stem_node.commitment {
                        child_values[i] = commitment_to_value(commitment);
                    }
                }

                 Some(NodeRef::Branch(branch_node)) => {
                    if let Some(commitment) = branch_node.commitment {
                        child_values[i] = commitment_to_value(commitment);
                    }
                }
            }
        }
        let commitment = commit_vector(&child_values, pk);  
        
        self.commitment = Some(commitment); 
        commitment
    }
}



impl StemNode {

    pub fn new(stem: Stem) -> Self {
        StemNode {
            stem,
            values: [const { None }; 256],
            commitment: None, 
        }
    }

    // commitment per gli StemNode: fa commitment ai 256 valori
    pub fn compute_commitment(&mut self, pk: &PublicKey) -> VectorCommitment { 
        let mut values_array = [[0u8; 48]; 256];
        
        for i in 0..256 {
            match self.values[i] {
                None => {}
                Some(value) => {
                    values_array[i] = value;
                }
            } 
        }       
        let commitment = commit_vector(&values_array, pk);  

        self.commitment= Some(commitment);
        commitment
    }
}

//------------------------------------------------------------
// VERKLE TREE impl

pub struct VerkleTree {
    root: BranchNode,
    pk: PublicKey,  
}

impl VerkleTree {

    pub fn new(pk: PublicKey) -> Self {
        let mut root = BranchNode::new();
        root.commitment = Some(commit_vector(&[[0u8; 48]; 256], &pk));
        VerkleTree { root, pk }
    }

    //recupero il commitment del tree
    pub fn get_root_commitment(&self) -> Option<VectorCommitment> {
        self.root.commitment
    }

    pub fn getter_pk(&self) -> &PublicKey {
        &self.pk
    }

    //NUOVO
    // converte 48 bytes in un commitment
    fn deserialize_commitment(bytes: &[u8]) -> VectorCommitment {
        let affine = G1Affine::deserialize_compressed(bytes).unwrap();
        VectorCommitment {
            inner: affine.into(),
        }
    }

   //recupera il valore associato a una chiave che la passo in input, se non esiste ritorna none
    pub fn get(&self, key: &Key) -> Option<Value> {
      
        let stem = get_stem(key);
        let suffix = get_suffix(key);
        
        let mut current_node = &self.root;

        for &byte in stem.iter() {
            //controlla se c'è un figlio all'indice 'byte'
            match &current_node.children[byte as usize] {
                None =>{
                         return None;
                }
                Some(NodeRef::Branch(branch)) => {
                    //navigo al branch successivo
                    current_node = branch;
                }
                Some(NodeRef::Stem(stem_node)) => {
                    return stem_node.values[suffix as usize];
                }
            }
        }
        None
    }


    //-------------------------------------INSERIMENTO--------------------------------------
    
    pub fn insert(&mut self, key: Key, value: Value, ricalcola: bool) -> Vec<ModifyNode> {
        let stem = get_stem(&key);
        let suffix = get_suffix(&key);

        if ricalcola {
            let mut mods = Vec::new();
            let pk = &self.pk;
            Self::insert_and_update(&mut self.root, &stem, 0, suffix, value, pk, String::new(), &mut mods);
            mods
        }else {
            // ricostruzione all'avvio: solo struttura, i commitment arrivano dopo dal DB
            Self::insert_recursive(&mut self.root, &stem, 0, suffix, value);
            Vec::new()
        }
    }
    
    fn insert_recursive(node: &mut BranchNode, stem: &Stem, level: usize, suffix: u8, value: Value) {
        let index = stem[level];
        let child_index = index as usize;

        match &mut node.children[child_index] {
        // se la posizione è vuota, bisogna creare il percorso
            None => {
                if level == 30 {
                    let mut stem_node = StemNode::new(*stem);
                    stem_node.values[suffix as usize] = Some(value);
                    node.children[child_index] = Some(NodeRef::Stem(Box::new(stem_node)));
                } else {
                    let mut new_branch = Box::new(BranchNode::new());
                    Self::insert_recursive(
                        &mut new_branch,
                        stem,
                        level + 1,
                        suffix,
                        value,
                    );
                    node.children[child_index] = Some(NodeRef::Branch(new_branch));
                }
            }

            // esiste già un BranchNode, scendo ricorsivamente
            Some(NodeRef::Branch(branch)) => {
                Self::insert_recursive(branch, stem, level + 1, suffix, value)
            }

            // esiste già uno StemNode, aggiorno il valore
            Some(NodeRef::Stem(stem_node)) => {
                stem_node.values[suffix as usize] = Some(value);
            }
        }
    }



    // chiamato solo una volta, all'avvio carica i commitment dal DB e li associa ai vari nodi dell'albero appena creato
    pub fn load_commitments_from_db(&mut self,nodes: Vec<(String, String, Vec<u8>)>,) {
        for (path, _node_type, bytes) in nodes {
            let commitment = Self::deserialize_commitment(&bytes);
            self.set_commitment_at_path(&path, commitment);
        }
    }

    // naviga il tree e imposta il commitment al path giusto
    fn set_commitment_at_path(&mut self, path: &str, commitment: VectorCommitment) {
        // path vuoto = root
        if path.is_empty() {
            self.root.commitment = Some(commitment);
            return;
        }

        // converte il path hex in indici es. "00aa" → [0, 170] 
        let indices: Vec<usize> = (0..path.len())
            .step_by(2)
            .map(|i| usize::from_str_radix(&path[i..i+2], 16).unwrap())
            .collect();

        let mut current = &mut self.root;

        //il for scorre gli indici[0, 170] con enumerate, che aggiuge la posizione:
        //depth=0, idx=0 // depth=1, idx=1
        for (depth, &idx) in indices.iter().enumerate() {
            let is_last = depth == indices.len() - 1;

            if is_last {
                match &mut current.children[idx] {
                    Some(NodeRef::Branch(branch)) => {
                        branch.commitment = Some(commitment);
                    }
                    Some(NodeRef::Stem(stem)) => {
                        stem.commitment = Some(commitment);
                    }
                    None => {}
                }
                return;
            }

            // scendi al prossimo livello
            match &mut current.children[idx] {
                Some(NodeRef::Branch(branch)) => {
                    current = branch;
                }
                _ => return,
            }
        }
    }

        
    
   

    fn insert_and_update(node: &mut BranchNode, stem: &Stem, level: usize, suffix: u8,value: Value, pk: &PublicKey, path: String,mods: &mut Vec<ModifyNode> ) {

        let idx = stem[level] as usize;
        let path_child = format!("{}{:02x}", path, idx);

        //prima cosa: accedo al commit seguendo il percorso che viene modificato
        let child_node= &node.children[idx];

        let old_commitment= match child_node{  //commitment del figlio del padre
            Some(NodeRef::Branch(b)) => b.commitment,
            Some(NodeRef::Stem(s)) =>s.commitment,
            None => None,
        };

        //o recupero il commitment del figlio e lo trasformo in value oppure se old_commitment restituisce un None allora vuol dire che non esiste quel figlio e 
        let old_commitment_v: Value = match old_commitment {
            Some(c) => commitment_to_value(c),
            None => [0u8; 48],
        };
        
        //se il figlio non esiste, lo creo con il commitment "vuoto" serve dopo per calcolare la differenza 
        if node.children[idx].is_none() {
            let vuoto = commit_vector(&[[0u8; 48]; 256], pk);
            node.children[idx] = Some(if level == 30 {
                let mut s = StemNode::new(*stem);
                s.commitment = Some(vuoto);
                NodeRef::Stem(Box::new(s))
            } else {
                let mut b = BranchNode::new();
                b.commitment = Some(vuoto);
                NodeRef::Branch(Box::new(b))
            });
        }

        //scendo, e ottengo cosa "vede" ora questo nodo nella posizione idx
        let new_commitment: Value = match node.children[idx].as_mut().unwrap() {
        
            NodeRef::Branch(b) => {
                Self::insert_and_update(b, stem, level + 1, suffix, value, pk, path_child.clone(), mods);
                commitment_to_value(b.commitment.unwrap()) //il risultato viene poi salvato in new_commitment
            }

            NodeRef::Stem(s) => {
                let old_v = s.values[suffix as usize].unwrap_or([0u8; 48]);
                s.values[suffix as usize] = Some(value);
                let c = update_commitment(s.commitment.unwrap(), suffix as usize, &old_v, &value, pk);
                s.commitment = Some(c);
                mods.push(ModifyNode {
                    path: path_child,
                    node_type: "stem".to_string(),
                    commitment_bytes: commitment_to_value(c),
                });
                commitment_to_value(c) // il risutato viene salvato in new_commitment
            }
        };

        
        let c = update_commitment(node.commitment.unwrap(), idx, &old_commitment_v, &new_commitment, pk);
        node.commitment = Some(c);

        mods.push(ModifyNode {
        path,
        node_type: "branch".to_string(),
        commitment_bytes: commitment_to_value(c),
        });

    }
    

    pub fn debug_branch_di_stem(&self, key: &Key) {
    let stem = get_stem(key);
    let mut cur = &self.root;
    for &b in stem.iter().take(30) {
        match &cur.children[b as usize] {
            Some(NodeRef::Branch(n)) => cur = n,
            _ => { println!("percorso interrotto"); return; }
        }
    }
    // cur = branch di livello 30, il padre dello stem
    let mut vals = [[0u8; 48]; 256];
    let mut figli = 0;
    for i in 0..256 {
        if let Some(NodeRef::Stem(s)) = &cur.children[i] {
            figli += 1;
            match s.commitment {
                Some(c) => vals[i] = commitment_to_value(c),
                None => println!("stem {:02x} senza commitment", i),
            }
        }
    }
    let ricalcolato = commit_vector(&vals, &self.pk);
    println!("figli: {}, salvato == ricalcolato? {}",
             figli, cur.commitment.map(|c| c.inner) == Some(ricalcolato.inner));
}

    // metodo chiamato x calcolare la prova e "inviare" <x,y,w> 
    pub fn prove(&self, key: &Key) -> Option<MembershipProof> {
        let stem = get_stem(key);
        let suffix = get_suffix(key);

        // raccoglie i nodi visitati durante la discesa + posizione
        let mut ls_nodi_visitati: Vec<(&BranchNode, usize)> = Vec::new();
        let mut steps: Vec<ProofStep> = Vec::new();
        let mut current_node = &self.root;


        //scorro l'array dello stem
        for &indice in stem.iter() {
            let child_indice = indice as usize;

            //se il figlio nella posizione 'indice' di current node è vuoto:
            match &current_node.children[child_indice] {
                None => {
                    println!("non esiste il percorso");
                return None;
                }

                Some(NodeRef::Branch(branch)) => {
                    ls_nodi_visitati.push((current_node, child_indice));
                    current_node = branch;
                }

                //se il figlio è uno stem node
                Some(NodeRef::Stem(stem_node)) => {
                    let value = stem_node.values[suffix as usize]?;
                    let commitment_stem = stem_node.commitment?;
                    
                    let mut values_array = [[0u8; 48]; 256];

                    //recupero tutti i valori dello stem
                    for i in 0..256 {
                        if let Some(v) = stem_node.values[i] {
                            values_array[i] = v;
                        }
                    }

                    //chiamo per la prova. in input: array dei valori e suffisso e pk
                    let witness_stem = prove_element(&values_array, suffix as usize, &self.pk);

                    steps.push(ProofStep {
                        commitment: commitment_stem, 
                        index: suffix as usize, 
                        value, 
                        witness: witness_stem, 
                    });

                    //recupero il commitment dello stemnode
                    let parent_commitment = current_node.commitment?;

                    let mut children_values_parent = [[0u8; 48]; 256];
                    for i in 0..256 {
                        match &current_node.children[i] {
                            None => {}

                            Some(NodeRef::Stem(s)) => {
                                if let Some(c) = s.commitment {
                                    children_values_parent[i] = commitment_to_value(c);
                                }
                            }
                            Some(NodeRef::Branch(b)) => {
                                if let Some(c) = b.commitment {
                                    children_values_parent[i] = commitment_to_value(c);
                                }
                            }
                        }
                    }
                        
                    let witness_parent = prove_element(&children_values_parent, child_indice, &self.pk);
                        
                    steps.push(ProofStep {
                        commitment: parent_commitment,
                        index: child_indice,
                        value: commitment_to_value(commitment_stem),
                        witness: witness_parent,
                    });
                    
                    let mut child_commitment_as_value = commitment_to_value(parent_commitment);

                        //scorro la lista dei nodi visitati al contrario
                    for (branch_node, idx) in ls_nodi_visitati.iter().rev() {
                          
                        let branch_commitment = branch_node.commitment?;
                        let mut children_values = [[0u8; 48]; 256];
                        for i in 0..256 {
                            match &branch_node.children[i] {
                                None => {}

                                Some(NodeRef::Stem(s)) => {
                                    if let Some(c) = s.commitment {
                                        children_values[i] = commitment_to_value(c);
                                    }
                                }
                                Some(NodeRef::Branch(b)) => {
                                    if let Some(c) = b.commitment {
                                        children_values[i] = commitment_to_value(c);
                                    }
                                }
                            }
                        }
                        // idx è la posizione del figlio (già noto) in questo branch_node, calcolo la prova
                        let witness = prove_element(&children_values, *idx, &self.pk);
                            
                            
                        steps.push(ProofStep {
                            commitment: branch_commitment,
                            index: *idx,
                            value: child_commitment_as_value,
                            witness,
                        });

                        child_commitment_as_value = commitment_to_value(branch_commitment);
                    }

                    return Some(MembershipProof { steps, value, key:*key });
                } 
            }
        }
        None
    }


    // verifica della prova
   pub fn verify_proof(proof: &MembershipProof, pk: &PublicKey, root: VectorCommitment, expected_key: Key) -> bool {
    if proof.key.as_ref() != expected_key.as_ref() { println!("FALLITO: chiave diversa"); return false; }
    if proof.steps.len() != 32 { println!("FALLITO: passi = {}", proof.steps.len()); return false; }

    for (j, step) in proof.steps.iter().enumerate() {
        if step.index != expected_key[31 - j] as usize {
            println!("FALLITO: indice al passo {} ({} invece di {})", j, step.index, expected_key[31 - j]);
            return false;
        }
    }

    if proof.steps.last().unwrap().commitment.inner != root.inner { println!("FALLITO: radice"); return false; }
    if proof.value != proof.steps.first().unwrap().value { println!("FALLITO: valore del primo passo"); return false; }

    for i in 1..proof.steps.len() {
        if proof.steps[i].value != commitment_to_value(proof.steps[i - 1].commitment) {
            println!("FALLITO: catena tra passo {} e {}", i - 1, i);
            return false;
        }
    }

    for (i, step) in proof.steps.iter().enumerate() {
        if !verify_element(step.commitment, step.index, step.value, step.witness.clone(), pk) {
            println!("FALLITO: pairing al passo {}", i);
            return false;
        }
    }
    true
}


        



}



#[derive(Debug, Clone)]
pub struct ProofStep {
    pub commitment: VectorCommitment, 
    pub index: usize,                 
    pub value: Value,                  
    pub witness: G1Projective,       
}

// la proof completa
#[derive(Debug, Clone)]
pub struct MembershipProof {
    pub steps: Vec<ProofStep>,  
    pub value: Value,  
    pub key: Key          
}
