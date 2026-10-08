pub mod types;
pub mod interpolate;
pub mod commit;
pub use types::VectorCommitment;
pub use commit::{commit_vector, prove_element, verify_element, commitment_to_value, update_commitment};