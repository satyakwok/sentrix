// trie/mod.rs - Sentrix — Binary Sparse Merkle Tree module

pub mod address;
pub mod cache;
pub mod node;
pub mod proof;
pub mod storage;
pub mod tree;

pub use address::{account_value_bytes, account_value_decode, address_to_key};
pub use node::{NULL_HASH, NodeHash, TrieNode, empty_hash, get_bit, hash_internal, hash_leaf};
pub use proof::MerkleProof;
pub use tree::SentrixTrie;
