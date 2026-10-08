use ark_bls12_381::Fr;
use ark_ff::Field;
use std::sync::OnceLock;


static BASI: OnceLock<Vec<Vec<Fr>>> = OnceLock::new();


//lagrange: serve per ottenere un polinomio che passa dai punti all'interno del vettore di valori 'children'
pub fn interpolate_with_lagrange(values: &[Fr; 256]) -> Vec<Fr> {
    
    let n = values.len();
    let basi = basi_lagrange();
    let mut coefficients = vec![Fr::from(0u64); n];
    
    for i in 0..n {
        let lagrange_poly = &basi[i];
        
        //moltiplicazione tra i coef di un unico polinomio base * y
        for j in 0..n {
            coefficients[j] += lagrange_poly[j] * values[i];
        }
    }
    coefficients
}

//chiamato una sola volta. verifico se vettore BASI è già stato popolato
fn basi_lagrange() -> &'static Vec<Vec<Fr>> {
    BASI.get_or_init(costruisci_basi)    
    }

fn costruisci_basi() -> Vec<Vec<Fr>> {
    let mut tabella = Vec::with_capacity(256);
    for i in 0..256 {
        tabella.push(compute_lagrange_basis(i, 256));
    }
    tabella
}

// crea il polinomio base L_i(x)
fn compute_lagrange_basis(i: usize, n: usize) -> Vec<Fr> {

    let mut poly = vec![Fr::from(1u64)]; // INIZIA DA "1"

    for j in 0..n {
        if j != i {
            let a = Fr::from(j as u64);
            // poly = poly * (x - j)
            poly = multiply_poly_by_linear(&poly, a); //poly viene aggiornato
            // primo ciclo 1 * (x - 1)
            // secondo ciclo (x - 1) * (x - 2)
        }
    }
    
    //faccio (i-j), qundi scorro j
   	let mut denominator = Fr::from(1u64);
    for j in 0..n {
            if j != i {
            let i_scalar = Fr::from(i as u64);
            let j_scalar = Fr::from(j as u64);
                denominator *= i_scalar - j_scalar; //viene aggiornato 
        }
    }
    
    // eseguo la divisione tra num e denominatore 
    let inv = denominator.inverse().expect("err");
    for j in 0..n {
        poly[j]*=inv
    }

    poly
}


// poly = [1]       -->  (1)
// poly = [-1, 1]   -->  (1x - 1)
// a è l'indice
fn multiply_poly_by_linear(poly: &[Fr], a: Fr) -> Vec<Fr> {
    let n = poly.len();
    let mut result = vec![Fr::from(0u64); n + 1]; 
    
    //  moltiplico per x
    for i in 0..n {
        result[i + 1] += poly[i];
    }

    //(9x^3 + 2x^2 - 3x -5) -> poly = [-5, -3, 2, 9] -> result = [0, -5, -3, 2, 9]

    //  moltiplico per -a
    for i in 0..n {
        result[i] =  result[i] - poly[i] * a;
    }

    result
}

