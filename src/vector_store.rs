//! # VectorStore Module
//!
//! This module provides functionality to store, serialize, deserialize, and search
//! high-dimensional vectors associated with user content. It supports building,
//! maintaining, and querying a nearest-neighbor search index for fast retrieval.

use hora::core::ann_index::{ANNIndex, SerializableIndex};
use hora::core::metrics::Metric;
use hora::index::hnsw_idx::HNSWIndex;
use hora::index::hnsw_params::HNSWParams;
use regex::Regex;
use rust_bert::pipelines::sentence_embeddings::{
    SentenceEmbeddingsBuilder, SentenceEmbeddingsModel,
};
use serde::{ser::SerializeStruct, Serialize, Serializer};
use std::collections::HashMap;
use std::error::Error;
use std::path::PathBuf;

use crate::config_dir;
use crate::brain::Memory;

/// A persistent vector database for mapping high-dimensional vectors to associated memory content.
///
/// `VectorStore` uses a HNSW index for approximate nearest neighbor (ANN) searches
/// and a transformer-based sentence embedding model to convert text into vectors.
///
/// It supports serialization, deserialization, building the search index,
/// and associating user messages or context with vectors.
pub struct VectorStore {
    /// Internal HNSWIndex for fast vector similarity search.
    pub index: HNSWIndex<f32, usize>,
    /// Dimensionality of vectors stored in the index.
    dimension: usize,
    /// Model used to embed text into vector representations.
    model: SentenceEmbeddingsModel,
    /// Counter used to assign a unique ID to each inserted vector.
    current_id: usize,
    /// Mapping of vector IDs to their corresponding memory content.
    id_to_memory: HashMap<usize, Memory>,
    /// A UUID derived from the session name for consistent serialization.
    uuid: u64
}

impl Serialize for VectorStore {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {

        let mut state = serializer.serialize_struct("VectorStore", 6)?;
        state.serialize_field("index", &self.index)?;
        state.serialize_field("dimension", &self.dimension)?;
        state.serialize_field("model", &0)?; // Model is not serialized
        state.serialize_field("current_id", &self.current_id)?;
        state.serialize_field("id_to_memory", &self.id_to_memory)?;
        state.serialize_field("uuid", &self.uuid)?;
        state.end()
    }
}

use std::{fmt, fs};

use serde::de::{self, Deserialize, Deserializer, MapAccess, SeqAccess, Visitor};

impl<'de> Deserialize<'de> for VectorStore {
    /// Custom serializer for `VectorStore`, storing essential fields while skipping the model.
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
            Uuid
        }

        impl<'de> Deserialize<'de> for Field {
            fn deserialize<D>(deserializer: D) -> Result<Field, D::Error>
            where
                D: Deserializer<'de>,
            {
                struct FieldVisitor;

                impl<'de> Visitor<'de> for FieldVisitor {
                    type Value = Field;

                    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                        formatter.write_str(
                            "`index` or `dimension` or `model` or `current_id` or `id_to_memory`, `uuid`",
                        )
                    }

                    fn visit_str<E>(self, value: &str) -> Result<Field, E>
                    where
                        E: de::Error,
                    {
                        match value {
                            "index" => Ok(Field::Index),
                            "dimension" => Ok(Field::Dimension),
                            "model" => Ok(Field::Model),
                            "current_id" => Ok(Field::CurrentId),
                            "id_to_memory" => Ok(Field::IdToMemory),
                            "uuid" => Ok(Field::Uuid),
                            _ => Err(de::Error::unknown_field(value, FIELDS)),
                        }
                    }
                }

                deserializer.deserialize_identifier(FieldVisitor)
            }
        }

        struct VectorStoreVisitor;

        impl<'de> Visitor<'de> for VectorStoreVisitor {
            type Value = VectorStore;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("struct VectorStore")
            }

            fn visit_seq<V>(self, mut seq: V) -> Result<VectorStore, V::Error>
            where
                V: SeqAccess<'de>,
            {
                let index = seq
                    .next_element()?
                    .ok_or_else(|| de::Error::invalid_length(0, &self))?;
                let dimension = seq
                    .next_element()?
                    .ok_or_else(|| de::Error::invalid_length(1, &self))?;
                let _model: usize = seq
                    .next_element()?
                    .ok_or_else(|| de::Error::invalid_length(1, &self))?;
                let current_id = seq
                    .next_element()?
                    .ok_or_else(|| de::Error::invalid_length(1, &self))?;
                let id_to_memory = seq
                    .next_element()?
                    .ok_or_else(|| de::Error::invalid_length(1, &self))?;
                let uuid: u64 = seq
                    .next_element()?
                    .ok_or_else(|| de::Error::invalid_length(1, &self))?;

                let vs = VectorStore::from_serialized(index, dimension, current_id, id_to_memory, uuid)
                    .unwrap();
                Ok(vs)
            }

            fn visit_map<V>(self, mut map: V) -> Result<VectorStore, V::Error>
            where
                V: MapAccess<'de>,
            {
                let mut index = None;
                let mut dimension = None;
                let mut model: Option<usize> = None;
                let mut current_id = None;
                let mut id_to_memory = None;
                let mut uuid: Option<u64> = None;
                while let Some(key) = map.next_key()? {
                    match key {
                        Field::Index => {
                            if index.is_some() {
                                return Err(de::Error::duplicate_field("index"));
                            }
                            index = Some(map.next_value()?);
                        }
                        Field::Dimension => {
                            if dimension.is_some() {
                                return Err(de::Error::duplicate_field("dimension"));
                            }
                            dimension = Some(map.next_value()?);
                        }
                        Field::Model => {
                            if model.is_some() {
                                return Err(de::Error::duplicate_field("model"));
                            }
                            model = Some(map.next_value()?);
                        }
                        Field::CurrentId => {
                            if current_id.is_some() {
                                return Err(de::Error::duplicate_field("current_id"));
                            }
                            current_id = Some(map.next_value()?);
                        }
                        Field::IdToMemory => {
                            if id_to_memory.is_some() {
                                return Err(de::Error::duplicate_field("id_to_memory"));
                            }
                            id_to_memory = Some(map.next_value()?);
                        }
                        Field::Uuid => {
                            if uuid.is_some() {
                                return Err(de::Error::duplicate_field("uuid"));
                            }
                            uuid = Some(map.next_value()?);
                        }
                    }
                }
                let index = index.ok_or_else(|| de::Error::missing_field("index"))?;
                let dimension = dimension.ok_or_else(|| de::Error::missing_field("dimension"))?;
                let _model = model.ok_or_else(|| de::Error::missing_field("model"))?;
                let current_id =
                    current_id.ok_or_else(|| de::Error::missing_field("current_id"))?;
                let id_to_memory =
                    id_to_memory.ok_or_else(|| de::Error::missing_field("id_to_memory"))?;
                let uuid =
                    uuid.ok_or_else(|| de::Error::missing_field("uuid"))?;

                let vs = VectorStore::from_serialized(index, dimension, current_id, id_to_memory, uuid)
                    .unwrap();
                Ok(vs)
            }
        }

        const FIELDS: &[&str] = &["index", "dimension", "model", "current_id", "id_to_memory", "uuid"];
        deserializer.deserialize_struct("VectorStore", FIELDS, VectorStoreVisitor)
    }
}

impl VectorStore {
    /// Create a new `VectorStore`.
    ///
    /// # Parameters
    /// - `dimension: usize`: Dimensionality of vectors to be stored.
    /// - `the_session_name: String`: Session name used to generate a unique identifier.
    ///
    /// # Returns
    /// - `Result<Self, Box<dyn Error>>`: Initialized VectorStore or error.
    pub fn new(dimension: usize, the_session_name: String) -> Result<Self, Box<dyn std::error::Error>> {
        let params = HNSWParams::default();
        let index = HNSWIndex::new(dimension, &params);
        let model = SentenceEmbeddingsBuilder::local("all-mini-lm-l12-v2")
            .create_model()
            .unwrap();

        let digest = sha256::digest(the_session_name);
        let mut uuid: u64 = 0;
        for byte in digest.as_bytes() {
            uuid += *byte as u64
        };

        Ok(Self {
            index,
            dimension,
            model,
            current_id: 0,
            id_to_memory: HashMap::new(), // Initialize the HashMap here
            uuid
        })
    }

    /// Serialize the vector store, saving the HNSW index and associated metadata to disk.
    ///
    /// # Parameters
    /// - `vector_store_path: &PathBuf`: Path to save the serialized metadata (YAML file).
    /// - `the_session_name: String`: Name of the session for consistent UUID generation.
    ///
    /// # Returns
    /// - `Result<(), Box<dyn Error>>`: Success or failure.
    pub fn serialize(&mut self, vector_store_path: &PathBuf, the_session_name: String) -> Result<(), Box<dyn Error>> {
        let digest = sha256::digest(the_session_name);
        let mut uuid: u64 = 0;
        for byte in digest.as_bytes() {
            uuid += *byte as u64
        };

        let index_file_name = format!("{}_hnsw_index.bin", uuid);
        let index_file = config_dir()?.join(index_file_name);
        self.index
            .dump(index_file.as_path().to_str().unwrap())
            .unwrap();
        let vector_store_string = serde_yaml::to_string(self)?;
        let _res = fs::write(vector_store_path, vector_store_string);

        Ok(())
    }

    /// Reconstruct a `VectorStore` from serialized data.
    ///
    /// # Parameters
    /// - `_index: HNSWIndex<f32, usize>`: Ignored (reloaded from file based on UUID).
    /// - `dimension: usize`: Vector dimensionality.
    /// - `current_id: usize`: Current ID counter.
    /// - `id_to_memory: HashMap<usize, Memory>`: ID-to-memory mapping.
    /// - `uuid: u64`: Unique identifier associated with the session.
    ///
    /// # Returns
    /// - `Result<Self, Box<dyn Error>>`: Reconstructed VectorStore or error.
    pub fn from_serialized(
        _index: HNSWIndex<f32, usize>,
        dimension: usize,
        current_id: usize,
        id_to_memory: HashMap<usize, Memory>,
        uuid: u64
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let model = SentenceEmbeddingsBuilder::local("all-mini-lm-l12-v2")
            .create_model()
            .unwrap();

        let index_file_name = format!("{}_hnsw_index.bin", uuid);
        let index_file = config_dir()?.join(index_file_name);
        let index = HNSWIndex::load(index_file.as_path().to_str().unwrap()).unwrap();

        Ok(Self {
            index,
            dimension,
            model,
            current_id: current_id,
            id_to_memory: id_to_memory, // Initialize the HashMap here
            uuid
        })
    }

    /// Add a new vector and associated memory to the store.
    ///
    /// # Parameters
    /// - `vector: Vec<f32>`: The embedded vector representation.
    /// - `memory: Memory`: The associated memory/content to store.
    ///
    /// # Returns
    /// - `Result<usize, &'static str>`: ID of the inserted vector or error if duplicate or dimension mismatch.
    pub fn add_vector_with_content(
        &mut self,
        vector: Vec<f32>,
        memory: Memory,
    ) -> Result<usize, &'static str> {
        if vector.len() != self.dimension {
            return Err("Vector dimension does not match the index dimension.");
        }

        let maybe_dup = self.index.search_nodes(&vector, 1);
        if maybe_dup.len() > 0 {
            let (_maybe_dup_node, maybe_dup_metric_val) = maybe_dup[0].clone();
            if maybe_dup_metric_val != 0 as f32 {
                let id = self.current_id;
                self.index
                    .add(&vector, id)
                    .map_err(|_| "Failed to add vector to the index.")?;

                self.id_to_memory.insert(id, memory); // Store the content associated with this vector

                self.current_id += 1;

                Ok(id)
            } else {
                Err("Vector already exists in index")
            }
        } else {
            let id = self.current_id;
            self.index
                .add(&vector, id)
                .map_err(|_| "Failed to add vector to the index.")?;

            self.id_to_memory.insert(id, memory); // Store the content associated with this vector

            self.current_id += 1;

            Ok(id)
        }
    }

    /// Retrieve memory content associated with a given vector ID.
    ///
    /// # Parameters
    /// - `id: usize`: The ID of the memory.
    ///
    /// # Returns
    /// - `Option<&Memory>`: Reference to memory if found, otherwise `None`.
    pub fn get_content_by_id(&self, id: usize) -> Option<&Memory> {
        self.id_to_memory.get(&id)
    }

    /// Build the underlying HNSW index to optimize nearest neighbor searches.
    ///
    /// # Returns
    /// - `Result<(), &'static str>`: Success or error if the build fails.
    pub fn build(&mut self) -> Result<(), &'static str> {
        self.index
            .build(Metric::Euclidean)
            .map_err(|_| "Failed to build the index.")
    }

    /// Perform a nearest neighbor search for a given vector.
    ///
    /// # Parameters
    /// - `vector: &[f32]`: Query vector.
    /// - `top_k: usize`: Number of top similar neighbors to return.
    ///
    /// # Returns
    /// - `Result<Vec<usize>, &'static str>`: List of matching vector IDs or error.
    pub fn search(&self, vector: &[f32], top_k: usize) -> Result<Vec<usize>, &'static str> {
        if vector.len() != self.dimension {
            return Err("Query vector dimension does not match the index dimension.");
        }

        Ok(self.index.search(vector, top_k))
    }

    /// Embed a given text into a vector representation using the loaded model.
    ///
    /// # Parameters
    /// - `text: &str`: Input text to be embedded.
    ///
    /// # Returns
    /// - `Result<Vec<f32>, Box<dyn Error>>`: Embedded vector or error.
    pub fn embed_text_to_vector(&self, text: &str) -> Result<Vec<f32>, Box<dyn std::error::Error>> {
        // Put your text into an array (you can add more sentences if needed)
        let sentences: Vec<String> = Self::tokenize_sentences(text);

        // Generate embeddings
        let embeddings = self.model.encode(&sentences)?;

        // Since it returns a 2D vector, we need to flatten it or select the first element
        // as each sentence corresponds to an embedding vector in the output
        let embedding_vector = embeddings.first().cloned().unwrap_or_default();

        Ok(embedding_vector)
    }

    /// Tokenize text into sentences, extracting code blocks and individual sentences separately.
    ///
    /// This function preserves code blocks and standard sentences for more meaningful embeddings.
    ///
    /// # Parameters
    /// - `text: &str`: Input text to tokenize.
    ///
    /// # Returns
    /// - `Vec<String>`: List of extracted sentences.
    pub fn tokenize_sentences(text: &str) -> Vec<String> {
        let mut sentences = Vec::new();
        let code_block_re = Regex::new(r"```([^`]+)```").unwrap();
        let sentence_re = Regex::new(r"(?s)[^.!?]+[.!?]").unwrap();

        // Extract code blocks as whole sentences
        let remaining_text = code_block_re.replace_all(text, |caps: &regex::Captures| {
            sentences.push(caps[1].trim().to_string());
            "".to_string() // Remove code block content from the remaining text
        });

        // Extract regular sentences from the non-code-block part of the text
        for cap in sentence_re.captures_iter(&remaining_text) {
            sentences.push(cap[0].trim().to_string());
        }

        // Check if there is an incomplete sentence at the end and add it to the sentences vector
        if let Some(last_char) = remaining_text.chars().last() {
            if last_char != '.' && last_char != '?' && last_char != '!' {
                sentences.push(remaining_text.trim().to_string());
            }
        }

        sentences
    }

    /// Calculate the Euclidean distance between two vectors.
    ///
    /// # Parameters
    /// - `a: Vec<f32>`: First vector.
    /// - `b: Vec<f32>`: Second vector.
    ///
    /// # Returns
    /// - `f32`: Euclidean distance.
    pub fn calc_euclidean_distance(a: Vec<f32>, b: Vec<f32>) -> f32 {
        let distance = a
            .iter()
            .enumerate()
            .fold(0 as f32, |mut accum, (pos, a_val)| {
                accum += (a_val - b[pos]).powi(2);
                accum
            });
        return distance.sqrt();
    }
}

#[cfg(test)]
mod tests {
    use async_openai::types::Role;

    use super::*;

    #[tokio::test]
    async fn test_vector_store() -> Result<(), Box<dyn std::error::Error>> {
        let mut store: VectorStore = VectorStore::new(384, "a_session_name".to_string())?;

        let sentences = vec![
            "Rust is pretty cool.",
            "I love programming.",
            "Coding is my passion.",
            "I enjoy writing code.",
            "Software development is fascinating.",
        ];

        for sentence in &sentences {
            let vector = store.embed_text_to_vector(sentence)?;
            let memory = Memory::new(Role::User, sentence.to_string());
            store.add_vector_with_content(vector, memory)?;
            // Fixed this line
        }

        store.build()?;

        let query_sentence = "Programming is love.";
        let query_vector = store.embed_text_to_vector(query_sentence)?;

        let neighbors = store.search(&query_vector, 1)?;

        assert_eq!(neighbors, vec![1]);

        Ok(())
    }
}
