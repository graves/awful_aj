//! # VectorStore
//!
//! Persistent embedding database for Awful Jade.
//!
//! This module provides a wrapper around a [HNSW](https://arxiv.org/abs/1603.09320)
//! approximate nearest-neighbor index (`hora` crate) plus a sentence embedding
//! model (`rust-bert`). It embeds text into 384-d vectors using the
//! `all-mini-lm-l12-v2` model, stores vectors with an ID↔memory mapping,
//! and performs fast semantic lookups.
//!
//! ## Responsibilities
//! - **Embedding**: Uses `all-mini-lm-l12-v2` to convert text into vectors.
//! - **Indexing**: Maintains a HNSW index for ANN queries.
//! - **Persistence**: Dumps the index to a binary file and metadata to YAML.
//! - **Association**: Links each vector to a [`Memory`](crate::brain::Memory).
//!
//! ## Serialization layout
//! - YAML contains: index snapshot (via `hora`), `dimension`, `current_id`,
//!   `id_to_memory`, and a stable `uuid`.
//! - The Transformer model **is not serialized**. It is reloaded from disk
//!   when deserializing via [`VectorStore::from_serialized`].
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

use hora::core::ann_index::{ANNIndex, SerializableIndex};
use hora::core::metrics::Metric;
use hora::index::hnsw_idx::HNSWIndex;
use hora::index::hnsw_params::HNSWParams;
use regex::Regex;
#[cfg(feature = "embed-rust-bert")]
use rust_bert::pipelines::sentence_embeddings::{
    SentenceEmbeddingsBuilder, SentenceEmbeddingsModel,
};
use serde::{Serialize, Serializer, ser::SerializeStruct};
use std::collections::HashMap;
use std::error::Error;
use std::path::PathBuf;

use crate::brain::Memory;
use crate::config_dir;

/// Persistent embedding store tied to a session.
///
/// Internally holds a HNSW index, a sentence embedding model,
/// and an ID→Memory map for recall.
pub struct VectorStore {
    /// ANN index for similarity search.
    pub index: HNSWIndex<f32, usize>,
    /// Dimensionality of vectors (usually 384 for MiniLM).
    dimension: usize,
    /// Transformer encoder for embeddings.
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
    /// because it’s heavy and resides on disk. See [`VectorStore::from_serialized`]
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
    /// sentence embedding model from `config_dir()/all-mini-lm-l12-v2`.
    ///
    /// If those artifacts are missing, deserialization will fail.
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
    /// - `dimension`: Dimensionality expected by the index and vectors (MiniLM is 384).
    /// - `the_session_name`: Used to derive a stable `uuid` for locating persisted index files.
    ///
    /// # Returns
    /// A ready-to-use `VectorStore`. You can immediately call
    /// [`embed_text_to_vector`], [`add_vector_with_content`], and then [`build`].
    ///
    /// # Errors
    /// Returns an error if the embedding model cannot be loaded from disk.
    ///
    /// # Example
    /// ```no_run
    /// # use awful_aj::vector_store::VectorStore;
    /// let vs = VectorStore::new(384, "demo".to_string()).unwrap();
    /// ```
    pub fn new(dimension: usize, the_session_name: String) -> Result<Self, Box<dyn Error>> {
        let index = HNSWIndex::new(dimension, &HNSWParams::default());
        let model = SentenceEmbeddingsBuilder::local("all-mini-lm-l12-v2")
            .create_model()
            .map_err(|e| format!("failed to load embedding model: {e}"))?;

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
    /// - If the model folder is missing or invalid.
    /// - If the HNSW binary cannot be found or fails to load.
    pub fn from_serialized(
        _index: HNSWIndex<f32, usize>,
        dimension: usize,
        current_id: usize,
        id_to_memory: HashMap<usize, Memory>,
        uuid: u64,
    ) -> Result<Self, Box<dyn Error>> {
        let model_root = Self::model_dir()?;
        if !model_root.join("config.json").exists() {
            return Err(format!("BERT model not found at {}", model_root.display()).into());
        }

        let model = SentenceEmbeddingsBuilder::local(&model_root)
            .create_model()
            .map_err(|e| {
                format!(
                    "failed to load sentence model from {}: {e}",
                    model_root.display()
                )
            })?;

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
    /// and **before** running [`search`], otherwise queries won’t see the new data.
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
    /// - If the index hasn’t been built yet, results may be empty or suboptimal.
    pub fn search(&self, vector: &[f32], top_k: usize) -> Result<Vec<usize>, &'static str> {
        if vector.len() != self.dimension {
            return Err("dimension mismatch");
        }
        Ok(self.index.search(vector, top_k))
    }

    /// Embed text into a dense vector using the loaded Transformer model.
    ///
    /// The text is first tokenized via [`tokenize_sentences`]. Embeddings are computed
    /// for each sentence/code block, and this function returns the **first** vector.
    ///
    /// # Parameters
    /// - `text`: Arbitrary input text (may contain code blocks in triple backticks).
    ///
    /// # Returns
    /// A single embedding vector (`Vec<f32>`). If the model returns multiple vectors,
    /// the first is chosen. If it returns none, an empty vector is returned.
    ///
    /// # Errors
    /// Propagates model inference errors.
    pub fn embed_text_to_vector(&self, text: &str) -> Result<Vec<f32>, Box<dyn Error>> {
        let sentences: Vec<String> = Self::tokenize_sentences(text);
        let embeddings = self.model.encode(&sentences)?;
        Ok(embeddings.first().cloned().unwrap_or_default())
    }

    /// Tokenize text into a list of sentences while preserving fenced code blocks.
    ///
    /// - Content inside triple backticks ```like this``` is captured as whole “sentences”.
    /// - Remaining text is split by punctuation (`.?!`).
    /// - A trailing fragment (no final punctuation) is kept as a last sentence.
    ///
    /// # Parameters
    /// - `text`: The raw input to split.
    ///
    /// # Returns
    /// A `Vec<String>` alternating code chunks and natural sentences, suitable
    /// for feeding to the embedding model.
    ///
    /// # Example
    /// ```
    /// # use awful_aj::vector_store::VectorStore;
    /// let parts = VectorStore::tokenize_sentences("Hello world! ```let x=1;``` Bye?");
    /// assert!(parts.iter().any(|s| s.contains("let x=1")));
    /// ```
    pub fn tokenize_sentences(text: &str) -> Vec<String> {
        let mut sentences = Vec::new();
        let code_block_re = Regex::new(r"```([^`]+)```").unwrap();
        let sentence_re = Regex::new(r"(?s)[^.!?]+[.!?]").unwrap();

        let remaining = code_block_re.replace_all(text, |caps: &regex::Captures| {
            sentences.push(caps[1].trim().to_string());
            "".to_string()
        });
        for cap in sentence_re.captures_iter(&remaining) {
            sentences.push(cap[0].trim().to_string());
        }
        if let Some(last) = remaining.chars().last() {
            if !['.', '?', '!'].contains(&last) {
                sentences.push(remaining.trim().to_string());
            }
        }
        sentences
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

    /// Resolve the on-disk directory that should contain `all-mini-lm-l12-v2`.
    ///
    /// This is `config_dir()/all-mini-lm-l12-v2`. The caller is responsible for
    /// ensuring the directory exists (see `ensure_all_mini()` in your crate root).
    ///
    /// # Errors
    /// Returns an error if the application’s config directory cannot be determined.
    fn model_dir() -> Result<PathBuf, Box<dyn Error>> {
        Ok(config_dir()?.join("all-mini-lm-l12-v2"))
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

#[cfg(feature = "embed-stub")]
mod doc_stub {
    pub struct SentenceEmbeddingsModel;
    impl SentenceEmbeddingsModel {
        pub fn encode(
            &self,
            inputs: &[String],
        ) -> Result<Vec<Vec<f32>>, Box<dyn std::error::Error>> {
            Ok(inputs
                .iter()
                .map(|s| {
                    let mut v = vec![0.0f32; 384];
                    for (i, b) in s.as_bytes().iter().enumerate() {
                        v[i % 384] += *b as f32 / 255.0;
                    }
                    v
                })
                .collect())
        }
    }
    pub struct SentenceEmbeddingsBuilder;
    impl SentenceEmbeddingsBuilder {
        pub fn local(_path: impl AsRef<std::path::Path>) -> Self {
            Self
        }
        pub fn create_model(self) -> Result<SentenceEmbeddingsModel, Box<dyn std::error::Error>> {
            Ok(SentenceEmbeddingsModel)
        }
    }
}
#[cfg(feature = "embed-stub")]
use doc_stub::{SentenceEmbeddingsBuilder, SentenceEmbeddingsModel};
