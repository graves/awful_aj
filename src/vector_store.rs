use std::collections::HashMap;  // Add this import for HashMap
use hora::core::ann_index::ANNIndex;
use hora::core::metrics::Metric;
use hora::index::hnsw_idx::HNSWIndex;
use hora::index::hnsw_params::HNSWParams;
use rust_bert::pipelines::sentence_embeddings::{
    SentenceEmbeddingsBuilder, SentenceEmbeddingsModel, SentenceEmbeddingsModelType,
};
use regex::Regex;

pub struct VectorStore {
    index: HNSWIndex<f32, usize>,
    dimension: usize,
    model: SentenceEmbeddingsModel,
    current_id: usize,
    id_to_content: HashMap<usize, String>,  // Added to hold the content mapping
}

impl VectorStore {
    pub async fn new(dimension: usize) -> Result<Self, Box<dyn std::error::Error>> {
        let params = HNSWParams::default();
        let index = HNSWIndex::new(dimension, &params);

        let model = tokio::task::spawn_blocking(|| {
            SentenceEmbeddingsBuilder::remote(SentenceEmbeddingsModelType::AllMiniLmL12V2)
                .create_model()
        })
        .await?
        .unwrap();

        Ok(Self {
            index,
            dimension,
            model,
            current_id: 0,
            id_to_content: HashMap::new(),  // Initialize the HashMap here
        })
    }

    pub fn add_vector_with_content(&mut self, vector: Vec<f32>, content: String) -> Result<usize, &'static str> {
        if vector.len() != self.dimension {
            return Err("Vector dimension does not match the index dimension.");
        }

        let id = self.current_id;
        self.index
            .add(&vector, id)
            .map_err(|_| "Failed to add vector to the index.")?;

        self.id_to_content.insert(id, content);  // Store the content associated with this vector

        self.current_id += 1;

        Ok(id)
    }

    pub fn get_content_by_id(&self, id: usize) -> Option<&String> {
        self.id_to_content.get(&id)
    }

    pub fn build(&mut self) -> Result<(), &'static str> {
        self.index
            .build(Metric::Euclidean)
            .map_err(|_| "Failed to build the index.")
    }

    pub fn search(&self, vector: &[f32], top_k: usize) -> Result<Vec<usize>, &'static str> {
        if vector.len() != self.dimension {
            return Err("Query vector dimension does not match the index dimension.");
        }
        Ok(self.index.search(vector, top_k))
    }

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

    pub fn tokenize_sentences(text: &str) -> Vec<String> {
        let mut sentences = Vec::new();
        let code_block_re = Regex::new(r"```([^`]+)```").unwrap();
        let sentence_re = Regex::new(r"(?s)[^.!?]+[.!?]").unwrap();
    
        // Extract code blocks as whole sentences
        let remaining_text = code_block_re.replace_all(text, |caps: &regex::Captures| {
            sentences.push(caps[1].trim().to_string());
            "".to_string()  // Remove code block content from the remaining text
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
    
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_vector_store() -> Result<(), Box<dyn std::error::Error>> {
        let mut store: VectorStore = VectorStore::new(384).await?;

        let sentences = vec![
            "Rust is pretty cool.",
            "I love programming.",
            "Coding is my passion.",
            "I enjoy writing code.",
            "Software development is fascinating.",
        ];

        for sentence in &sentences {
            let vector = store.embed_text_to_vector(sentence)?;
            store.add_vector_with_content(vector, sentence.clone().to_string())?;  // Fixed this line
        }

        store.build()?;

        let query_sentence = "Programming is love.";
        let query_vector = store.embed_text_to_vector(query_sentence)?;

        let neighbors = store.search(&query_vector, 1)?;

        assert_eq!(neighbors, vec![1]);

        Ok(())
    }
}
