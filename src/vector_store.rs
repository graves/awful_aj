//! # VectorStore
//!
//! Persistent embedding database for Awful Jade.
//!
//! This module provides a wrapper around a [HNSW](https://arxiv.org/abs/1603.09320)
//! approximate nearest-neighbor index (`hora` crate) plus a sentence embedding
//! model using Candle (pure Rust ML framework). It embeds text into 384-d vectors,
//! stores vectors with an ID↔memory mapping, and performs fast semantic lookups.
//!
//! ## Responsibilities
//! - **Embedding**: Uses all-MiniLM-L6-v2 model via Candle to convert text into vectors.
//! - **Indexing**: Maintains a HNSW index for ANN queries.
//! - **Persistence**: Dumps the index to a binary file and metadata to YAML.
//! - **Association**: Links each vector to a [`Memory`](crate::brain::Memory).
//!
//! ## Serialization layout
//! - YAML contains: index snapshot (via `hora`), `dimension`, `current_id`,
//!   `id_to_memory`, and a stable `uuid`.
//! - The model is reloaded from cache when deserializing.
//!
//! ## Quick Example
//! ```no_run
//! use awful_aj::vector_store::VectorStore;
//! use awful_aj::brain::Memory;
//! use async_openai::types::Role;
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! let mut vs = VectorStore::new(384, "session_name".to_string())?;
//! let v = vs.embed_text_to_vector("Rust is great!")?;
//! vs.add_vector_with_content(v, Memory::new(Role::User, "Rust is great!".into()))?;
//! vs.build()?;
//! let q = vs.embed_text_to_vector("I love Rust!")?;
//! let ids = vs.search(&q, 1)?;
//! println!("Top match IDs: {ids:?}");
//! # Ok(()) }
//! ```

use candle_core::{DType, Device, Tensor};
use candle_nn::VarBuilder;
use candle_transformers::models::bert::{BertModel, Config, DTYPE};
use hf_hub::{api::sync::Api, Repo, RepoType};
use hora::core::ann_index::{ANNIndex, SerializableIndex};
use hora::core::metrics::Metric;
use hora::index::hnsw_idx::HNSWIndex;
use hora::index::hnsw_params::HNSWParams;
use serde::{Serialize, Serializer, ser::SerializeStruct};
use std::collections::HashMap;
use std::error::Error;
use std::path::PathBuf;
use tokenizers::Tokenizer;

use crate::brain::Memory;
use crate::config_dir;

/// Sentence embeddings model using Candle (pure Rust)
pub struct SentenceEmbeddingsModel {
    model: BertModel,
    tokenizer: Tokenizer,
    device: Device,
}

impl SentenceEmbeddingsModel {
    /// Load the model from Hugging Face Hub
    pub fn load() -> Result<Self, Box<dyn Error>> {
        let device = Device::Cpu;
        let model_id = "sentence-transformers/all-MiniLM-L6-v2";
        let revision = "main";
        
        // Download model files from Hugging Face
        let repo = Repo::with_revision(model_id.to_string(), RepoType::Model, revision.to_string());
        let api = Api::new()?;
        let api_repo = api.repo(repo);
        
        let config_filename = api_repo.get("config.json")?;
        let tokenizer_filename = api_repo.get("tokenizer.json")?;
        let weights_filename = api_repo.get("model.safetensors")?;
        
        // Load config
        let config = std::fs::read_to_string(config_filename)?;
        let config: Config = serde_json::from_str(&config)?;
        
        // Load tokenizer
        let tokenizer = Tokenizer::from_file(tokenizer_filename)
            .map_err(|e| format!("Failed to load tokenizer: {}", e))?;
        
        // Load weights
        let vb = unsafe { VarBuilder::from_mmaped_safetensors(&[weights_filename], DTYPE, &device)? };
        let model = BertModel::load(vb, &config)?;
        
        Ok(Self {
            model,
            tokenizer,
            device,
        })
    }
    
    /// Encode text into an embedding
    pub fn encode(&self, text: &str) -> Result<Vec<f32>, Box<dyn Error>> {
        // Tokenize with automatic truncation at 512 tokens
        let tokens = self.tokenizer
            .encode(text, true)
            .map_err(|e| format!("Tokenization error: {}", e))?;
        
        let token_ids = Tensor::new(tokens.get_ids(), &self.device)?
            .unsqueeze(0)?;
        let token_type_ids = Tensor::new(tokens.get_type_ids(), &self.device)?
            .unsqueeze(0)?;
        
        // Run model inference
        let output = self.model.forward(&token_ids, &token_type_ids, None)?;
        
        // Mean pooling
        let embedding = self.mean_pooling(&output, tokens.get_attention_mask())?;
        
        // Normalize
        let embedding = self.normalize(&embedding)?;
        
        // Convert to Vec<f32>
        let embedding_vec = embedding.to_vec1::<f32>()?;
        
        Ok(embedding_vec)
    }
    
    /// Mean pooling over token embeddings, considering attention mask
    fn mean_pooling(&self, embeddings: &Tensor, attention_mask: &[u32]) -> Result<Tensor, Box<dyn Error>> {
        // embeddings shape: [batch_size, seq_len, hidden_size] = [1, seq_len, 384]
        // attention_mask: [seq_len]
        // We need mask shape: [1, seq_len, 1] for proper broadcasting
        
        let mask = Tensor::new(attention_mask, &self.device)?
            .to_dtype(DType::F32)?
            .unsqueeze(0)?  // [1, seq_len]
            .unsqueeze(2)?; // [1, seq_len, 1]
        
        // Multiply embeddings by mask (broadcasting happens automatically)
        let masked = embeddings.broadcast_mul(&mask)?;
        
        // Sum across sequence dimension (dim=1)
        let sum = masked.sum(1)?;  // [1, 384]
        
        // Count valid tokens (sum mask across sequence dimension)
        let count = mask.sum(1)?.clamp(1f32, f32::INFINITY)?;  // [1, 1]
        
        // Divide to get mean
        let mean = sum.broadcast_div(&count)?;  // [1, 384]
        
        // Squeeze to get [384]
        let mean = mean.squeeze(0)?;
        
        Ok(mean)
    }
    
    /// L2 normalize the embedding vector
    fn normalize(&self, tensor: &Tensor) -> Result<Tensor, Box<dyn Error>> {
        let norm = tensor.sqr()?.sum_all()?.sqrt()?;
        let normalized = tensor.broadcast_div(&norm)?;
        Ok(normalized)
    }
}

/// Builder for sentence embeddings model
pub struct SentenceEmbeddingsBuilder;

impl SentenceEmbeddingsBuilder {
    pub fn local(_path: impl AsRef<std::path::Path>) -> Self {
        Self
    }
    
    pub fn create_model(self) -> Result<SentenceEmbeddingsModel, Box<dyn std::error::Error>> {
        SentenceEmbeddingsModel::load()
    }
}

/// Persistent embedding store tied to a session.
///
/// Internally holds a HNSW index, a sentence embedding model,
/// and an ID→Memory map for recall.
pub struct VectorStore {
    /// ANN index for similarity search.
    pub index: HNSWIndex<f32, usize>,
    /// Dimensionality of vectors (384 for MiniLM-L6).
    dimension: usize,
    /// Sentence embedding model.
    model: SentenceEmbeddingsModel,
    /// Auto-incrementing ID counter for new vectors.
    current_id: usize,
    /// Mapping from ID → associated memory.
    id_to_memory: HashMap<usize, Memory>,
    /// UUID derived from session name (stable across reloads).
    uuid: u64,
}

impl Serialize for VectorStore {
    /// Custom serializer for `VectorStore`.
    ///
    /// The sentence embedding model is **not** serialized (only a dummy `0` is written),
    /// because it's loaded from Hugging Face Hub. See [`VectorStore::from_serialized`]
    /// for the complementary logic that reloads the model at runtime.
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut state = serializer.serialize_struct("VectorStore", 6)?;
        state.serialize_field("index", &self.index)?;
        state.serialize_field("dimension", &self.dimension)?;
        state.serialize_field("model", &0)?; // skip model
        state.serialize_field("current_id", &self.current_id)?;
        state.serialize_field("id_to_memory", &self.id_to_memory)?;
        state.serialize_field("uuid", &self.uuid)?;
        state.end()
    }
}

use serde::de::{self, Deserialize, Deserializer, MapAccess, SeqAccess, Visitor};
use std::{fmt, fs};

impl<'de> Deserialize<'de> for VectorStore {
    /// Custom deserializer for `VectorStore`.
    ///
    /// Rehydrates the HNSW index from `<uuid>_hnsw_index.bin` and reloads the
    /// sentence embedding model from Hugging Face Hub.
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        enum Field {
            Index,
            Dimension,
            Model,
            CurrentId,
            IdToMemory,
            Uuid,
        }

        impl<'de> Deserialize<'de> for Field {
            fn deserialize<D2: Deserializer<'de>>(d: D2) -> Result<Self, D2::Error> {
                struct F;
                impl<'de> Visitor<'de> for F {
                    type Value = Field;
                    fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                        f.write_str(
                            "`index`|`dimension`|`model`|`current_id`|`id_to_memory`|`uuid`",
                        )
                    }
                    fn visit_str<E: de::Error>(self, v: &str) -> Result<Field, E> {
                        Ok(match v {
                            "index" => Field::Index,
                            "dimension" => Field::Dimension,
                            "model" => Field::Model,
                            "current_id" => Field::CurrentId,
                            "id_to_memory" => Field::IdToMemory,
                            "uuid" => Field::Uuid,
                            _ => return Err(E::unknown_field(v, &FIELDS)),
                        })
                    }
                }
                d.deserialize_identifier(F)
            }
        }

        struct VectorStoreVisitor;

        impl<'de> Visitor<'de> for VectorStoreVisitor {
            type Value = VectorStore;

            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str("struct VectorStore")
            }

            fn visit_seq<V: SeqAccess<'de>>(self, mut seq: V) -> Result<Self::Value, V::Error> {
                let index = seq
                    .next_element()?
                    .ok_or_else(|| de::Error::invalid_length(0, &self))?;
                let dimension = seq
                    .next_element()?
                    .ok_or_else(|| de::Error::invalid_length(1, &self))?;
                let _model: usize = seq
                    .next_element()?
                    .ok_or_else(|| de::Error::invalid_length(2, &self))?;
                let current_id = seq
                    .next_element()?
                    .ok_or_else(|| de::Error::invalid_length(3, &self))?;
                let id_to_memory = seq
                    .next_element()?
                    .ok_or_else(|| de::Error::invalid_length(4, &self))?;
                let uuid = seq
                    .next_element()?
                    .ok_or_else(|| de::Error::invalid_length(5, &self))?;
                VectorStore::from_serialized(index, dimension, current_id, id_to_memory, uuid)
                    .map_err(|e| de::Error::custom(e.to_string()))
            }

            fn visit_map<V: MapAccess<'de>>(self, mut map: V) -> Result<Self::Value, V::Error> {
                let (
                    mut index,
                    mut dimension,
                    mut model,
                    mut current_id,
                    mut id_to_memory,
                    mut uuid,
                ) = (None, None, None::<usize>, None, None, None::<u64>);

                while let Some(key) = map.next_key()? {
                    match key {
                        Field::Index => index = Some(map.next_value()?),
                        Field::Dimension => dimension = Some(map.next_value()?),
                        Field::Model => model = Some(map.next_value()?),
                        Field::CurrentId => current_id = Some(map.next_value()?),
                        Field::IdToMemory => id_to_memory = Some(map.next_value()?),
                        Field::Uuid => uuid = Some(map.next_value()?),
                    }
                }
                let (index, dimension, _model, current_id, id_to_memory, uuid) = (
                    index.ok_or_else(|| de::Error::missing_field("index"))?,
                    dimension.ok_or_else(|| de::Error::missing_field("dimension"))?,
                    model.ok_or_else(|| de::Error::missing_field("model"))?,
                    current_id.ok_or_else(|| de::Error::missing_field("current_id"))?,
                    id_to_memory.ok_or_else(|| de::Error::missing_field("id_to_memory"))?,
                    uuid.ok_or_else(|| de::Error::missing_field("uuid"))?,
                );

                VectorStore::from_serialized(index, dimension, current_id, id_to_memory, uuid)
                    .map_err(|e| de::Error::custom(e.to_string()))
            }
        }

        const FIELDS: &[&str] = &[
            "index",
            "dimension",
            "model",
            "current_id",
            "id_to_memory",
            "uuid",
        ];
        deserializer.deserialize_struct("VectorStore", FIELDS, VectorStoreVisitor)
    }
}

impl VectorStore {
    /// Create an empty store with a fresh HNSW index and a loaded sentence embedding model.
    ///
    /// # Parameters
    /// - `dimension`: Dimensionality expected by the index and vectors (384 for MiniLM-L6).
    /// - `the_session_name`: Used to derive a stable `uuid` for locating persisted index files.
    ///
    /// # Returns
    /// A ready-to-use `VectorStore`. You can immediately call
    /// [`embed_text_to_vector`], [`add_vector_with_content`], and then [`build`].
    ///
    /// # Errors
    /// Returns an error if the embedding model cannot be loaded.
    ///
    /// # Example
    /// ```no_run
    /// # use awful_aj::vector_store::VectorStore;
    /// let vs = VectorStore::new(384, "demo".to_string()).unwrap();
    /// ```
    pub fn new(dimension: usize, the_session_name: String) -> Result<Self, Box<dyn Error>> {
        let index = HNSWIndex::new(dimension, &HNSWParams::default());
        let model = SentenceEmbeddingsBuilder::local("").create_model()?;

        let digest = sha256::digest(the_session_name);
        let uuid = digest.as_bytes().iter().map(|b| *b as u64).sum();

        Ok(Self {
            index,
            dimension,
            model,
            current_id: 0,
            id_to_memory: HashMap::new(),
            uuid,
        })
    }

    /// Serialize metadata to YAML and dump the HNSW index to a binary file.
    ///
    /// - YAML is written to `vector_store_path`.
    /// - The index is saved to `config_dir()/<uuid>_hnsw_index.bin` (derived from `the_session_name`).
    ///
    /// # Parameters
    /// - `vector_store_path`: Where to write the YAML metadata.
    /// - `the_session_name`: Used to recompute `uuid` to name the index file.
    ///
    /// # Errors
    /// - I/O failures while writing YAML or index file.
    /// - Serialization problems (unlikely unless fields contain invalid data).
    ///
    /// # Example
    /// ```no_run
    /// # use awful_aj::vector_store::VectorStore;
    /// # fn f()->Result<(),Box<dyn std::error::Error>>{
    /// let mut vs = VectorStore::new(384, "s".into())?;
    /// vs.serialize(&std::path::PathBuf::from("vector_store.yaml"), "s".into())?;
    /// # Ok(())}
    /// ```
    pub fn serialize(
        &mut self,
        vector_store_path: &PathBuf,
        the_session_name: String,
    ) -> Result<(), Box<dyn Error>> {
        let digest = sha256::digest(the_session_name);
        let uuid: u64 = digest.as_bytes().iter().map(|b| *b as u64).sum();

        let index_file = config_dir()?.join(format!("{}_hnsw_index.bin", uuid));
        self.index.dump(index_file.to_str().unwrap())?;

        let yaml = serde_yaml::to_string(self)?;
        fs::write(vector_store_path, yaml)?;
        Ok(())
    }

    /// Reconstruct a `VectorStore` from YAML metadata and a persisted HNSW index.
    ///
    /// The deserializer passes the (ignored) `index` snapshot, plus the fields necessary
    /// to reload: `dimension`, `current_id`, `id_to_memory`, and `uuid`.
    ///
    /// # Parameters
    /// - `_index`: Ignored; the index is reloaded from disk using `uuid`.
    /// - `dimension`: Vector dimensionality (must match the saved index).
    /// - `current_id`: Restores the next ID to assign.
    /// - `id_to_memory`: Restored ID→Memory mapping.
    /// - `uuid`: Used to find `<uuid>_hnsw_index.bin` under `config_dir()`.
    ///
    /// # Errors
    /// - If the HNSW binary cannot be found or fails to load.
    /// - If the model cannot be loaded from Hugging Face Hub.
    pub fn from_serialized(
        _index: HNSWIndex<f32, usize>,
        dimension: usize,
        current_id: usize,
        id_to_memory: HashMap<usize, Memory>,
        uuid: u64,
    ) -> Result<Self, Box<dyn Error>> {
        let model = SentenceEmbeddingsBuilder::local("").create_model()?;

        let index_file = config_dir()?.join(format!("{}_hnsw_index.bin", uuid));
        let index = HNSWIndex::load(index_file.to_str().unwrap())?;

        Ok(Self {
            index,
            dimension,
            model,
            current_id,
            id_to_memory,
            uuid,
        })
    }

    /// Add a vector and its associated memory to the index and map.
    ///
    /// # Parameters
    /// - `vector`: A vector of length `dimension`.
    /// - `memory`: The [`Memory`] to associate with this vector ID.
    ///
    /// # Returns
    /// The assigned integer ID for this vector.
    ///
    /// # Errors
    /// - Returns `"dimension mismatch"` if `vector.len() != self.dimension`.
    /// - Returns `"add failed"` if the HNSW index rejects the insert (rare).
    ///
    /// # Notes
    /// You must call [`build`] before queries reflect new inserts.
    pub fn add_vector_with_content(
        &mut self,
        vector: Vec<f32>,
        memory: Memory,
    ) -> Result<usize, &'static str> {
        if vector.len() != self.dimension {
            return Err("dimension mismatch");
        }
        let id = self.current_id;
        self.index.add(&vector, id).map_err(|_| "add failed")?;
        self.id_to_memory.insert(id, memory);
        self.current_id += 1;
        Ok(id)
    }

    /// Look up the stored memory by internal vector ID.
    ///
    /// Returns `None` if the ID is unknown or was not inserted.
    pub fn get_content_by_id(&self, id: usize) -> Option<&Memory> {
        self.id_to_memory.get(&id)
    }

    /// Finalize (build) the HNSW index.
    ///
    /// Must be called **after** a batch of `add_vector_with_content` operations
    /// and **before** running [`search`], otherwise queries won't see the new data.
    ///
    /// # Errors
    /// Returns `"build failed"` if the index fails to finalize.
    pub fn build(&mut self) -> Result<(), &'static str> {
        self.index
            .build(Metric::Euclidean)
            .map_err(|_| "build failed")
    }

    /// Query the index for the `top_k` nearest vectors to `vector`.
    ///
    /// # Parameters
    /// - `vector`: Query vector; must have length `dimension`.
    /// - `top_k`: Number of nearest IDs to return.
    ///
    /// # Returns
    /// A `Vec<usize>` of IDs sorted by increasing distance (best first).
    ///
    /// # Errors
    /// - `"dimension mismatch"` if `vector.len() != self.dimension`.
    /// - If the index hasn't been built yet, results may be empty or suboptimal.
    pub fn search(&self, vector: &[f32], top_k: usize) -> Result<Vec<usize>, &'static str> {
        if vector.len() != self.dimension {
            return Err("dimension mismatch");
        }
        Ok(self.index.search(vector, top_k))
    }

    /// Embed text into a dense vector using the loaded embedding model.
    ///
    /// The text is tokenized and embedded directly. If the text exceeds 512 tokens,
    /// it will be automatically truncated by the tokenizer.
    ///
    /// # Parameters
    /// - `text`: Arbitrary input text to embed.
    ///
    /// # Returns
    /// A 384-dimensional embedding vector (`Vec<f32>`).
    ///
    /// # Errors
    /// Propagates model inference errors.
    pub fn embed_text_to_vector(&self, text: &str) -> Result<Vec<f32>, Box<dyn Error>> {
        self.model.encode(text)
    }

    /// Compute Euclidean distance between two equal-length vectors.
    ///
    /// # Parameters
    /// - `a`: First vector.
    /// - `b`: Second vector (must be the same length as `a`).
    ///
    /// # Returns
    /// The Euclidean distance: `sqrt(Σ (a[i] - b[i])^2)`.
    ///
    /// # Panics
    /// Panics if `b.len() < a.len()` (index out of bounds). Caller should ensure equal length.
    pub fn calc_euclidean_distance(a: Vec<f32>, b: Vec<f32>) -> f32 {
        a.iter()
            .enumerate()
            .map(|(i, av)| (av - b[i]).powi(2))
            .sum::<f32>()
            .sqrt()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_openai::types::Role;

    #[tokio::test]
    async fn test_vector_store() -> Result<(), Box<dyn Error>> {
        let mut store = VectorStore::new(384, "test_session".to_string())?;
        let sents = ["Rust is cool.", "I love programming."];
        for s in sents {
            let v = store.embed_text_to_vector(s)?;
            store.add_vector_with_content(v, Memory::new(Role::User, s.to_string()))?;
        }
        store.build()?;
        let qv = store.embed_text_to_vector("Programming is fun.")?;
        let neighbors = store.search(&qv, 1)?;
        assert!(!neighbors.is_empty());
        Ok(())
    }
}
