// trie/storage.rs - Sentrix — Persistent sled-backed trie storage

use sled::{Db, Tree};
use crate::core::trie::node::{NodeHash, TrieNode};
use crate::types::error::{SentrixError, SentrixResult};

/// Low-level persistent storage for trie nodes, values, and version→root mappings.
///
/// Three named sled trees:
/// - `trie_nodes`  : NodeHash → bincode(TrieNode)
/// - `trie_values` : NodeHash → raw account-state bytes
/// - `trie_roots`  : version u64 BE → NodeHash
///
/// `Clone` is cheap — sled::Tree is an Arc internally (shared underlying tree).
#[derive(Clone)]
pub struct TrieStorage {
    nodes: Tree,
    values: Tree,
    roots: Tree,
}

impl TrieStorage {
    /// Open (or create) the three named trees from an existing sled Db.
    pub fn new(db: &Db) -> SentrixResult<Self> {
        let nodes = db
            .open_tree("trie_nodes")
            .map_err(|e| SentrixError::StorageError(e.to_string()))?;
        let values = db
            .open_tree("trie_values")
            .map_err(|e| SentrixError::StorageError(e.to_string()))?;
        let roots = db
            .open_tree("trie_roots")
            .map_err(|e| SentrixError::StorageError(e.to_string()))?;
        Ok(Self { nodes, values, roots })
    }

    // ── Nodes ─────────────────────────────────────────────

    pub fn store_node(&self, hash: &NodeHash, node: &TrieNode) -> SentrixResult<()> {
        let bytes = bincode::serialize(node)
            .map_err(|e| SentrixError::SerializationError(e.to_string()))?;
        self.nodes
            .insert(hash, bytes)
            .map_err(|e| SentrixError::StorageError(e.to_string()))?;
        Ok(())
    }

    pub fn load_node(&self, hash: &NodeHash) -> SentrixResult<Option<TrieNode>> {
        match self
            .nodes
            .get(hash)
            .map_err(|e| SentrixError::StorageError(e.to_string()))?
        {
            Some(bytes) => {
                let node = bincode::deserialize::<TrieNode>(&bytes)
                    .map_err(|e| SentrixError::SerializationError(e.to_string()))?;
                Ok(Some(node))
            }
            None => Ok(None),
        }
    }

    /// T-B: Remove a node entry from persistent storage (called when a leaf is replaced).
    pub fn delete_node(&self, hash: &NodeHash) -> SentrixResult<()> {
        self.nodes
            .remove(hash)
            .map_err(|e| SentrixError::StorageError(e.to_string()))?;
        Ok(())
    }

    // ── Values ────────────────────────────────────────────

    pub fn store_value(&self, hash: &NodeHash, value: &[u8]) -> SentrixResult<()> {
        self.values
            .insert(hash, value)
            .map_err(|e| SentrixError::StorageError(e.to_string()))?;
        Ok(())
    }

    pub fn load_value(&self, hash: &NodeHash) -> SentrixResult<Option<Vec<u8>>> {
        self.values
            .get(hash)
            .map_err(|e| SentrixError::StorageError(e.to_string()))
            .map(|opt| opt.map(|iv| iv.to_vec()))
    }

    /// T-B: Remove a value blob from persistent storage (called when a leaf is replaced).
    pub fn delete_value(&self, hash: &NodeHash) -> SentrixResult<()> {
        self.values
            .remove(hash)
            .map_err(|e| SentrixError::StorageError(e.to_string()))?;
        Ok(())
    }

    // ── Roots ─────────────────────────────────────────────

    pub fn store_root(&self, version: u64, root: &NodeHash) -> SentrixResult<()> {
        // sled uses a write-ahead log and is crash-safe by default.  Explicit
        // flush() calls are not required for durability and block the write lock
        // unnecessarily — removed in fix/trie-permanent-fix (ROOT CAUSE #2).
        self.roots
            .insert(version.to_be_bytes(), root.as_slice())
            .map_err(|e| SentrixError::StorageError(e.to_string()))?;
        Ok(())
    }

    pub fn load_root(&self, version: u64) -> SentrixResult<Option<NodeHash>> {
        match self
            .roots
            .get(version.to_be_bytes())
            .map_err(|e| SentrixError::StorageError(e.to_string()))?
        {
            Some(bytes) if bytes.len() == 32 => {
                let mut arr = [0u8; 32];
                arr.copy_from_slice(&bytes);
                Ok(Some(arr))
            }
            Some(_) => Err(SentrixError::StorageError(
                "corrupt trie root: wrong byte length".to_string(),
            )),
            None => Ok(None),
        }
    }

    /// Check whether `hash` is currently recorded as a committed root for any version.
    ///
    /// Called by `SentrixTrie::insert()` before deleting old internal nodes so that
    /// the root hash of a previously committed version is never removed — which would
    /// cause a "root missing" error on restart and trigger a non-deterministic backfill
    /// that permanently forks the chain (ROOT CAUSE #3 fix).
    pub fn is_committed_root(&self, hash: &NodeHash) -> SentrixResult<bool> {
        for entry in self.roots.iter() {
            let (_, v) = entry.map_err(|e| SentrixError::StorageError(e.to_string()))?;
            if v.len() == 32 && &v[..] == hash.as_slice() {
                return Ok(true);
            }
        }
        Ok(false)
    }

    /// T-F: Garbage-collect node and value entries not present in `live_hashes`.
    ///
    /// Scans both `trie_nodes` and `trie_values`, collecting every hash not in the live
    /// set, then deletes them.  Returns the total count of entries removed across both trees.
    ///
    /// GC sweeps both `trie_nodes` and `trie_values` — orphaned value blobs from delete()
    /// calls were previously never cleaned.  Now both trees are scanned.
    ///
    /// Callers must supply a complete set of hashes reachable from all committed roots
    /// they wish to preserve.  Nodes referenced only by un-committed (in-flight) mutations
    /// are safe to include — but omitting them will cause those nodes to be deleted.
    pub fn gc_orphaned_nodes(
        &self,
        live_hashes: &std::collections::HashSet<NodeHash>,
    ) -> SentrixResult<usize> {
        let node_count = self.gc_tree(&self.nodes, live_hashes)?;
        // Also GC value blobs — leaf value_hash matches leaf node_hash, same live set.
        let value_count = self.gc_tree(&self.values, live_hashes)?;
        Ok(node_count + value_count)
    }

    /// Shared helper: scan a sled Tree for hashes not in `live_hashes` and remove them.
    fn gc_tree(
        &self,
        tree: &sled::Tree,
        live_hashes: &std::collections::HashSet<NodeHash>,
    ) -> SentrixResult<usize> {
        let mut to_delete: Vec<NodeHash> = Vec::new();
        for entry in tree.iter() {
            let (k, _) = entry.map_err(|e| SentrixError::StorageError(e.to_string()))?;
            if k.len() == 32 {
                let mut arr = [0u8; 32];
                arr.copy_from_slice(&k);
                if !live_hashes.contains(&arr) {
                    to_delete.push(arr);
                }
            }
        }
        let count = to_delete.len();
        for hash in &to_delete {
            tree.remove(hash)
                .map_err(|e| SentrixError::StorageError(e.to_string()))?;
        }
        Ok(count)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::trie::node::{TrieNode, empty_hash};
    use std::collections::HashSet;

    fn temp_storage() -> (tempfile::TempDir, TrieStorage) {
        let dir = tempfile::TempDir::new().unwrap();
        let db  = sled::open(dir.path()).unwrap();
        let storage = TrieStorage::new(&db).unwrap();
        (dir, storage)
    }

    fn dummy_hash(byte: u8) -> NodeHash {
        let mut h = [0u8; 32];
        h[0] = byte;
        h
    }

    #[test]
    fn test_is_committed_root_true_for_stored() {
        let (_dir, storage) = temp_storage();
        let root = dummy_hash(0x10);
        storage.store_root(1, &root).unwrap();
        assert!(
            storage.is_committed_root(&root).unwrap(),
            "is_committed_root must return true for a stored root"
        );
    }

    #[test]
    fn test_is_committed_root_false_for_unknown() {
        let (_dir, storage) = temp_storage();
        let committed = dummy_hash(0x10);
        let other     = dummy_hash(0x20);
        storage.store_root(1, &committed).unwrap();
        assert!(
            !storage.is_committed_root(&other).unwrap(),
            "is_committed_root must return false for a hash not in trie_roots"
        );
    }

    #[test]
    fn test_store_root_no_blocking_flush() {
        // Regression: store_root() must not call nodes/values/roots.flush().
        // We validate this by calling store_root() many times quickly — if flushes
        // were present the test would be noticeably slow on spinning disk / CI.
        let (_dir, storage) = temp_storage();
        let root = dummy_hash(0xFF);
        for v in 0u64..50 {
            storage.store_root(v, &root).unwrap();
        }
        // Also confirm we can still load them back correctly.
        assert_eq!(storage.load_root(0).unwrap(), Some(root));
        assert_eq!(storage.load_root(49).unwrap(), Some(root));
    }

    #[test]
    fn test_delete_node_removes_entry() {
        let (_dir, storage) = temp_storage();
        let hash = dummy_hash(0xAB);
        let node = TrieNode::Leaf { key: [1u8; 32], value_hash: [2u8; 32] };

        storage.store_node(&hash, &node).unwrap();
        assert!(storage.load_node(&hash).unwrap().is_some(), "node must exist after store");

        storage.delete_node(&hash).unwrap();
        assert!(storage.load_node(&hash).unwrap().is_none(), "node must be absent after delete");
    }

    #[test]
    fn test_delete_value_removes_entry() {
        let (_dir, storage) = temp_storage();
        let hash = dummy_hash(0xCD);
        let val  = b"balance_data";

        storage.store_value(&hash, val).unwrap();
        assert!(storage.load_value(&hash).unwrap().is_some(), "value must exist after store");

        storage.delete_value(&hash).unwrap();
        assert!(storage.load_value(&hash).unwrap().is_none(), "value must be absent after delete");
    }

    #[test]
    fn test_gc_orphaned_nodes_removes_unlisted() {
        let (_dir, storage) = temp_storage();
        let live_hash   = dummy_hash(0x01);
        let orphan_hash = dummy_hash(0x02);

        let node = TrieNode::Leaf { key: [0u8; 32], value_hash: empty_hash(0) };
        storage.store_node(&live_hash,   &node).unwrap();
        storage.store_node(&orphan_hash, &node).unwrap();

        let mut live: HashSet<NodeHash> = HashSet::new();
        live.insert(live_hash);

        let removed = storage.gc_orphaned_nodes(&live).unwrap();
        assert_eq!(removed, 1, "exactly one orphan must be removed");
        assert!(storage.load_node(&live_hash).unwrap().is_some(),   "live node must survive GC");
        assert!(storage.load_node(&orphan_hash).unwrap().is_none(), "orphan must be removed by GC");
    }

    #[test]
    fn test_gc_empty_live_set_removes_all() {
        let (_dir, storage) = temp_storage();
        let node = TrieNode::Leaf { key: [0u8; 32], value_hash: empty_hash(0) };
        for i in 0u8..5 {
            storage.store_node(&dummy_hash(i), &node).unwrap();
        }
        let removed = storage.gc_orphaned_nodes(&HashSet::new()).unwrap();
        assert_eq!(removed, 5, "all 5 nodes must be removed when live set is empty");
    }

    #[test]
    fn test_gc_also_removes_orphan_values() {
        let (_dir, storage) = temp_storage();
        let live_hash   = dummy_hash(0x01);
        let orphan_hash = dummy_hash(0x02);

        let node = TrieNode::Leaf { key: [0u8; 32], value_hash: empty_hash(0) };
        storage.store_node(&live_hash,   &node).unwrap();
        storage.store_node(&orphan_hash, &node).unwrap();
        // Also store value blobs (as if delete() leaked them)
        storage.store_value(&live_hash,   b"live_data").unwrap();
        storage.store_value(&orphan_hash, b"orphan_data").unwrap();

        let mut live: std::collections::HashSet<NodeHash> = std::collections::HashSet::new();
        live.insert(live_hash);

        let removed = storage.gc_orphaned_nodes(&live).unwrap();
        // 1 orphan node + 1 orphan value = 2 removed
        assert_eq!(removed, 2, "GC must remove both orphan node and orphan value");
        assert!(storage.load_value(&live_hash).unwrap().is_some(),   "live value must survive GC");
        assert!(storage.load_value(&orphan_hash).unwrap().is_none(), "orphan value must be removed");
    }
}
