use crate::*;

use snarkos_errors::storage::StorageError;
use snarkos_objects::{
    dpc::{DPCTransactions, Transaction},
    BlockHeader,
    BlockHeaderHash,
};
use snarkos_utilities::bytes::FromBytes;

use parking_lot::RwLock;
use std::{
    fs,
    marker::PhantomData,
    path::{Path, PathBuf},
    sync::Arc,
};

pub struct BlockStorage<T: Transaction> {
    pub latest_block_height: RwLock<u32>,
    pub storage: Arc<Storage>,
    _transaction: PhantomData<T>,
}

impl<T: Transaction> BlockStorage<T> {
    /// Create a new blockchain storage.
    pub fn new() -> Result<Arc<Self>, StorageError> {
        let mut path = std::env::current_dir()?;
        path.push("../../db");

        let genesis = "00000000000000000000000000000000000000000000000000000000000000008c8d4f393f39c063c40a617c6e2584e6726448c4c0f7da7c848bfa573e628388fbf1285e00000000ffffffffff7f00005e4401000101000000010000000000000000000000000000000000000000000000000000000000000000ffffffff04010000000100e1f505000000001976a914ef5392fc02643be8b98f6aaca5c1ffaab238916a88ac".into();

        BlockStorage::open_at_path(path, genesis)
    }

    /// Open the blockchain storage at a particular path.
    pub fn open_at_path<P: AsRef<Path>>(path: P, genesis: String) -> Result<Arc<Self>, StorageError> {
        fs::create_dir_all(path.as_ref()).map_err(|err| StorageError::Message(err.to_string()))?;

        match Storage::open_cf(path, NUM_COLS) {
            Ok(storage) => Self::get_latest_state(storage, genesis),
            Err(err) => return Err(err),
        }
    }

    /// Get the latest state of the storage.
    pub fn get_latest_state(_storage: Storage, _genesis: String) -> Result<Arc<Self>, StorageError> {
        //        let value = storage.get(&Key::Meta(KEY_BEST_BLOCK_NUMBER))?;
        //
        //        match value {
        //            Some(val) => Ok(Arc::new(Self {
        //                latest_block_height: RwLock::new(bytes_to_u32(val)),
        //                storage: Arc::new(storage),
        //            })),
        //            None => {
        //                // Add genesis block to database
        //
        //                let block_storage = Self {
        //                    latest_block_height: RwLock::new(0),
        //                    storage: Arc::new(storage),
        //                };
        //
        //                let genesis_block = Block::deserialize(&hex::decode(genesis)?).unwrap();
        //
        //                block_storage.insert_and_commit(genesis_block)?;
        //
        //                Ok(Arc::new(block_storage))
        //            }
        //        }
        unimplemented!()
    }

    /// Get the latest block height of the chain.
    pub fn get_latest_block_height(&self) -> u32 {
        *self.latest_block_height.read()
    }

    /// Get the latest number of blocks in the chain.
    pub fn get_block_count(&self) -> u32 {
        *self.latest_block_height.read() + 1
    }

    /// Destroy the storage given a path.
    pub fn destroy_storage(path: PathBuf) -> Result<(), StorageError> {
        Storage::destroy_storage(path)
    }

    /// Retrieve a value given a key.
    pub(crate) fn get(&self, col: u32, key: &Vec<u8>) -> Result<Vec<u8>, StorageError> {
        match self.storage.get(col, key)? {
            Some(data) => Ok(data),
            None => Err(StorageError::MissingValue(hex::encode(key))),
        }
    }

    // KEY VALUE GETTERS ===========================================================================

    /// Get the stored memory pool transactions.
    pub fn get_memory_pool(&self) -> Result<Vec<u8>, StorageError> {
        Ok(self.get(COL_META, &KEY_MEMORY_POOL.as_bytes().to_vec())?)
    }

    /// Store the memory pool transactions.
    pub fn store_to_memory_pool(&self, transactions_serialized: Vec<u8>) -> Result<(), StorageError> {
        let op = Op::Insert {
            col: COL_META,
            key: KEY_MEMORY_POOL.as_bytes().to_vec(),
            value: transactions_serialized,
        };
        self.storage.write(DatabaseTransaction(vec![op]))
    }

    /// Get the stored old connected peers.
    pub fn get_peer_book(&self) -> Result<Vec<u8>, StorageError> {
        Ok(self.get(COL_META, &KEY_PEER_BOOK.as_bytes().to_vec())?)
    }

    /// Store the connected peers.
    pub fn store_to_peer_book(&self, peers_serialized: Vec<u8>) -> Result<(), StorageError> {
        let op = Op::Insert {
            col: COL_META,
            key: KEY_PEER_BOOK.as_bytes().to_vec(),
            value: peers_serialized,
        };
        self.storage.write(DatabaseTransaction(vec![op]))
    }

    /// Get a block header given the block hash.
    pub fn get_block_header(&self, block_hash: &BlockHeaderHash) -> Result<BlockHeader, StorageError> {
        match self.storage.get(COL_BLOCK_HEADER, &block_hash.0)? {
            Some(block_header_bytes) => Ok(BlockHeader::read(&block_header_bytes[..])?),
            None => Err(StorageError::MissingBlockHeader(block_hash.to_string())),
        }
    }

    /// Get the block hash given a block number.
    pub fn get_block_hash(&self, block_num: u32) -> Result<BlockHeaderHash, StorageError> {
        match self.storage.get(COL_BLOCK_LOCATOR, &block_num.to_le_bytes())? {
            Some(block_header_hash) => Ok(BlockHeaderHash::new(block_header_hash)),
            None => Err(StorageError::MissingBlockHash(block_num)),
        }
    }

    /// Get the block num given a block hash.
    pub fn get_block_num(&self, block_hash: &BlockHeaderHash) -> Result<u32, StorageError> {
        match self.storage.get(COL_BLOCK_LOCATOR, &block_hash.0)? {
            Some(block_num_bytes) => {
                let mut block_num = [0u8; 4];
                block_num.copy_from_slice(&block_num_bytes[0..4]);

                Ok(u32::from_le_bytes(block_num))
            }
            None => Err(StorageError::MissingBlockNumber(block_hash.to_string())),
        }
    }

    /// Get the list of transaction ids given a block hash.
    pub fn get_block_transactions(&self, block_hash: &BlockHeaderHash) -> Result<DPCTransactions<T>, StorageError> {
        match self.storage.get(COL_BLOCK_TRANSACTIONS, &block_hash.0)? {
            Some(encoded_block_transactions) => Ok(DPCTransactions::read(&encoded_block_transactions[..])?),
            None => Err(StorageError::MissingBlockTransactions(block_hash.to_string())),
        }
    }

    /// Find the potential child block given a parent block header.
    pub fn get_child_hash(&self, _parent_header: &BlockHeaderHash) -> Result<BlockHeaderHash, StorageError> {
        unimplemented!()
    }

    /// Get a transaction given the transaction id.
    pub fn get_transaction(&self, transaction_id: &Vec<u8>) -> Result<Option<T>, StorageError> {
        match self.storage.get(COL_TRANSACTION_LOCATION, &transaction_id)? {
            Some(transaction_locator) => {
                let transaction_location = TransactionLocation::read(&transaction_locator[..])?;
                let block_transactions =
                    self.get_block_transactions(&BlockHeaderHash(transaction_location.block_hash))?;
                Ok(Some(block_transactions.0[transaction_location.index as usize].clone()))
            }
            None => Ok(None),
        }
    }
}
