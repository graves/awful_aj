//! # Semantic Memory with HNSW Vector Search
//!
//! This module implements **Awful Jade's long-term semantic memory system** using
//! HNSW (Hierarchical Navigable Small World) approximate nearest neighbor search
//! combined with sentence embeddings. It enables efficient semantic similarity
//! search over conversation history and documents.
//!
//! ## Overview
//!
//! The vector store provides three core capabilities:
//!
//! 1. **Text Embedding**: Convert text to 384-dimensional vectors using `all-MiniLM-L6-v2`
//! 2. **Similarity Search**: Fast approximate nearest neighbor lookup via HNSW index
//! 3. **Persistence**: Save/load index and memory mappings to disk
//!
//! ## Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────┐
//! │                    VectorStore                           │
//! │                                                          │
//! │  ┌────────────────────┐        ┌────────────────────┐  │
//! │  │  Embedding Model   │        │   HNSW Index       │  │
//! │  │  (all-MiniLM-L6)   │        │   (hora crate)     │  │
//! │  └────────┬───────────┘        └─────────┬──────────┘  │
//! │           │                               │             │
//! │           ▼                               ▼             │
//! │    Text → [384-d vector] ──────→  [ID, distance]       │
//! │                                           │             │
//! │                                           ▼             │
//! │                              ┌────────────────────────┐ │
//! │                              │   ID → Memory Map      │ │
//! │                              │   HashMap<usize, Mem>  │ │
//! │                              └────────────────────────┘ │
//! └─────────────────────────────────────────────────────────┘
//! ```
//!
//! ## Core Components
//!
//! ### [`VectorStore`]
//!
//! The main facade that combines embedding, indexing, and retrieval:
//!
//! - **Embedding**: `embed_text_to_vector()` converts strings to vectors
//! - **Indexing**: `add_vector_with_content()` stores vectors with associated memories
//! - **Search**: `search()` finds k-nearest neighbors by Euclidean distance
//! - **Persistence**: `serialize()`/`from_serialized()` for disk storage
//!
//! ### [`SentenceEmbeddingsModel`]
//!
//! Pure Rust sentence transformer using Candle ML framework:
//!
//! - Model: `all-MiniLM-L6-v2` (BERT-based)
//! - Dimensions: 384
//! - Size: ~90MB
//! - Pooling: Mean pooling with L2 normalization
//!
//! ### HNSW Index
//!
//! Hierarchical graph structure from `hora` crate:
//!
//! - **Algorithm**: HNSW (state-of-the-art ANN)
//! - **Metric**: Euclidean distance
//! - **Complexity**: O(log N) search time
//! - **Parameters**: M=12, ef_construction=200 (default)
//!
//! ## Embedding Model Details
//!
//! The `all-MiniLM-L6-v2` model is automatically downloaded from HuggingFace Hub
//! on first use. It's a distilled BERT model optimized for semantic similarity:
//!
//! | Property | Value |
//! |----------|-------|
//! | Architecture | BERT (6 layers) |
//! | Parameters | 22.7M |
//! | Embedding Size | 384 dimensions |
//! | Max Sequence Length | 512 tokens |
//! | Training | Sentence transformers distillation |
//! | Performance | ~85% of full BERT at 10% size |
//!
//! **Cache Location**: HuggingFace Hub cache (`~/.cache/huggingface/hub/` on Linux/macOS)
//!
//! ## Persistence Format
//!
//! The vector store serializes to two files in the config directory:
//!
//! ```text
//! <config_dir>/<session_hash>_vector_store.yaml   # Metadata + ID→Memory map
//! <config_dir>/<session_hash>_hnsw_index.bin      # Binary HNSW graph
//! ```
//!
//! The YAML file contains:
//! - `dimension`: Vector dimensionality (384)
//! - `current_id`: Next available ID
//! - `id_to_memory`: HashMap of ID → [`Memory`](crate::brain::Memory)
//! - `uuid`: Session identifier (hash of session name)
//!
//! The binary file contains the HNSW index structure (serialized via `hora`).
//!
//! ## Usage Patterns
//!
//! ### Creating and Populating a Vector Store
//!
//! ```no_run
//! use awful_aj::vector_store::VectorStore;
//! use awful_aj::brain::Memory;
//! use async_openai::types::Role;
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! // Create new vector store for session
//! let mut vs = VectorStore::new(384, "my-session".to_string())?;
//!
//! // Add memories with automatic embedding
//! let text1 = "Rust is a systems programming language";
//! let vec1 = vs.embed_text_to_vector(text1)?;
//! vs.add_vector_with_content(
//!     vec1,
//!     Memory::new(Role::User, text1.to_string())
//! )?;
//!
//! let text2 = "HNSW is a graph-based ANN algorithm";
//! let vec2 = vs.embed_text_to_vector(text2)?;
//! vs.add_vector_with_content(
//!     vec2,
//!     Memory::new(Role::Assistant, text2.to_string())
//! )?;
//!
//! // Build index (required before search)
//! vs.build()?;
//!
//! // Persist to disk
//! let path = std::path::PathBuf::from("vector_store.yaml");
//! vs.serialize(&path, "my-session".to_string())?;
//! # Ok(())
//! # }
//! ```
//!
//! ### Semantic Search
//!
//! ```no_run
//! # use awful_aj::vector_store::VectorStore;
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! # let mut vs = VectorStore::new(384, "my-session".to_string())?;
//! # vs.build()?;
//! // Search for similar memories
//! let query = "What is Rust?";
//! let query_vec = vs.embed_text_to_vector(query)?;
//! let top_ids = vs.search(&query_vec, 5)?; // Get top 5 matches
//!
//! // Retrieve associated memories
//! for id in top_ids {
//!     if let Some(memory) = vs.get_content_by_id(id) {
//!         println!("Match {}: {}", id, memory.content);
//!     }
//! }
//! # Ok(())
//! # }
//! ```
//!
//! ### Loading from Disk
//!
//! Loading a vector store requires deserializing the YAML metadata and loading
//! the binary HNSW index. Use `VectorStore::from_serialized()` with the appropriate
//! parameters from the YAML file.
//!
//! ```no_run
//! use awful_aj::vector_store::VectorStore;
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! // Create a new vector store for this session
//! let mut vs = VectorStore::new(384, "my-session".to_string())?;
//!
//! // After populating and building the index, it can be searched
//! let query_vec = vs.embed_text_to_vector("example query")?;
//! let results = vs.search(&query_vec, 3)?;
//! # Ok(())
//! # }
//! ```
//!
//! ## Integration with Brain
//!
//! The vector store works alongside the [`Brain`](crate::brain::Brain) to provide
//! a two-tier memory system:
//!
//! ```text
//! User Query
//!     │
//!     ▼
//! VectorStore.search()  ← Semantic search in long-term memory
//!     │
//!     ▼
//! Top-K relevant memories
//!     │
//!     ▼
//! Brain.add_memory()    ← Inject into working memory
//!     │
//!     ▼
//! LLM with enriched context
//! ```
//!
//! ## Performance Characteristics
//!
//! | Operation | Time Complexity | Notes |
//! |-----------|----------------|-------|
//! | `embed_text_to_vector()` | O(n) | n = text length, ~20-50ms |
//! | `add_vector_with_content()` | O(1) | Constant time insertion |
//! | `build()` | O(N log N) | N = total vectors |
//! | `search()` | O(log N) | HNSW approximate search |
//! | `serialize()` | O(N) | Linear in index size |
//!
//! **Memory Usage**: ~1KB per stored vector (384 floats + metadata)
//!
//! ## Similarity Thresholds
//!
//! The HNSW index uses **Euclidean distance** as the similarity metric. After
//! L2 normalization, typical distance ranges:
//!
//! - **< 0.3**: Very similar (near-duplicates)
//! - **0.3 - 0.7**: Semantically related
//! - **0.7 - 1.0**: Loosely related
//! - **> 1.0**: Unrelated
//!
//! The search doesn't apply a distance threshold—it returns the k-nearest neighbors
//! regardless of absolute distance. Callers can filter results by distance if needed.
//!
//! ## Error Handling
//!
//! Most methods return `Result<T, Box<dyn Error>>` to propagate various error types:
//!
//! - **Model loading errors**: Network issues, cache corruption
//! - **Embedding errors**: Text too long (> 512 tokens), encoding failures
//! - **Index errors**: Build before search, invalid dimensionality
//! - **I/O errors**: Disk full, permission denied during serialization
//!
//! ## See Also
//!
//! - [`crate::brain::Brain`] - Short-term working memory
//! - [`crate::brain::Memory`] - Memory items stored in the index
//! - [HNSW Paper](https://arxiv.org/abs/1603.09320) - Algorithm details
//! - [all-MiniLM-L6-v2](https://huggingface.co/sentence-transformers/all-MiniLM-L6-v2) - Model card

use candle_core::{DType, Device, Tensor};
use candle_nn::VarBuilder;
use candle_transformers::models::bert::{BertModel, Config, DTYPE};
use hf_hub::{Repo, RepoType, api::sync::Api};
use hora::core::ann_index::{ANNIndex, SerializableIndex};
use hora::core::metrics::Metric;
use hora::index::hnsw_idx::HNSWIndex;
use hora::index::hnsw_params::HNSWParams;
use indicatif::{ProgressBar, ProgressStyle};
use serde::{Serialize, Serializer, ser::SerializeStruct};
use std::collections::HashMap;
use std::error::Error;
use std::path::PathBuf;
use std::time::Duration;
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
        let pb = ProgressBar::new_spinner();
        pb.set_style(
            ProgressStyle::with_template("{spinner:.cyan} {msg}")
                .unwrap()
                .tick_strings(&["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"]),
        );
        pb.enable_steady_tick(Duration::from_millis(80));

        let device = Device::Cpu;
        let model_id = "sentence-transformers/all-MiniLM-L6-v2";
        let revision = "main";

        // Download model files from Hugging Face
        let repo = Repo::with_revision(model_id.to_string(), RepoType::Model, revision.to_string());
        let api = Api::new()?;
        let api_repo = api.repo(repo);

        pb.set_message("Downloading config.json...");
        let config_filename = api_repo.get("config.json")?;

        pb.set_message("Downloading tokenizer.json...");
        let tokenizer_filename = api_repo.get("tokenizer.json")?;

        pb.set_message("Downloading model.safetensors (~90MB)...");
        let weights_filename = api_repo.get("model.safetensors")?;

        pb.set_message("Loading model configuration...");
        let config = std::fs::read_to_string(config_filename)?;
        let config: Config = serde_json::from_str(&config)?;

        pb.set_message("Loading tokenizer...");
        let tokenizer = Tokenizer::from_file(tokenizer_filename)
            .map_err(|e| format!("Failed to load tokenizer: {}", e))?;

        pb.set_message("Loading model weights...");
        let vb =
            unsafe { VarBuilder::from_mmaped_safetensors(&[weights_filename], DTYPE, &device)? };
        let model = BertModel::load(vb, &config)?;

        pb.finish_with_message("✓ Embedding model loaded");

        Ok(Self {
            model,
            tokenizer,
            device,
        })
    }

    /// Encode text into an embedding
    pub fn encode(&self, text: &str) -> Result<Vec<f32>, Box<dyn Error>> {
        // Tokenize with automatic truncation at 512 tokens
        let tokens = self
            .tokenizer
            .encode(text, true)
            .map_err(|e| format!("Tokenization error: {}", e))?;

        let token_ids = Tensor::new(tokens.get_ids(), &self.device)?.unsqueeze(0)?;
        let token_type_ids = Tensor::new(tokens.get_type_ids(), &self.device)?.unsqueeze(0)?;

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
    fn mean_pooling(
        &self,
        embeddings: &Tensor,
        attention_mask: &[u32],
    ) -> Result<Tensor, Box<dyn Error>> {
        // embeddings shape: [batch_size, seq_len, hidden_size] = [1, seq_len, 384]
        // attention_mask: [seq_len]
        // We need mask shape: [1, seq_len, 1] for proper broadcasting

        let mask = Tensor::new(attention_mask, &self.device)?
            .to_dtype(DType::F32)?
            .unsqueeze(0)? // [1, seq_len]
            .unsqueeze(2)?; // [1, seq_len, 1]

        // Multiply embeddings by mask (broadcasting happens automatically)
        let masked = embeddings.broadcast_mul(&mask)?;

        // Sum across sequence dimension (dim=1)
        let sum = masked.sum(1)?; // [1, 384]

        // Count valid tokens (sum mask across sequence dimension)
        let count = mask.sum(1)?.clamp(1f32, f32::INFINITY)?; // [1, 1]

        // Divide to get mean
        let mean = sum.broadcast_div(&count)?; // [1, 384]

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

    #[test]
    fn test_vector_store_is_send_sync() {
        fn assert_send<T: Send>() {}
        fn assert_sync<T: Sync>() {}

        assert_send::<VectorStore>();
        assert_sync::<VectorStore>();
    }

    #[test]
    fn test_sentence_embeddings_model_is_send_sync() {
        fn assert_send<T: Send>() {}
        fn assert_sync<T: Sync>() {}

        assert_send::<SentenceEmbeddingsModel>();
        assert_sync::<SentenceEmbeddingsModel>();
    }
}
