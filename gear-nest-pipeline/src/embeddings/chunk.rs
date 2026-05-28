//! Chunking strategies.
//!
//! Two distinct strategies per SPEC interview talking point on chunking:
//!
//! * **Sentence-boundary** for product specs and descriptions — preserves
//!   attribute/value pairs like "Weight: 14 oz." across chunk boundaries.
//! * **Fixed-size with overlap** for free-form review prose — 256 tokens with
//!   32-token overlap so conclusions that span a boundary aren't truncated.
//!
//! Token counts here are word-based approximations, not BPE; the embedding
//! model handles its own tokenization downstream.

const REVIEW_CHUNK_WORDS: usize = 256;
const REVIEW_OVERLAP_WORDS: usize = 32;

pub fn sentence_chunks(text: &str) -> Vec<String> {
    text.split_terminator(['.', '!', '?'])
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(|s| {
            // Re-attach the period so downstream "Weight: 14 oz." stays intact.
            if s.ends_with('.') || s.ends_with('!') || s.ends_with('?') {
                s.to_string()
            } else {
                format!("{s}.")
            }
        })
        .collect()
}

pub fn review_chunks(body: &str) -> Vec<String> {
    let words: Vec<&str> = body.split_whitespace().collect();
    if words.is_empty() {
        return Vec::new();
    }
    if words.len() <= REVIEW_CHUNK_WORDS {
        return vec![words.join(" ")];
    }

    let step = REVIEW_CHUNK_WORDS - REVIEW_OVERLAP_WORDS;
    let mut out: Vec<String> = Vec::new();
    let mut start = 0;
    while start < words.len() {
        let end = (start + REVIEW_CHUNK_WORDS).min(words.len());
        out.push(words[start..end].join(" "));
        if end == words.len() {
            break;
        }
        start += step;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sentence_chunks_preserves_spec_lines() {
        let chunks = sentence_chunks("Weight: 14 oz. Insulation: 800-fill. Capacity: 60L.");
        assert_eq!(chunks.len(), 3);
        assert_eq!(chunks[0], "Weight: 14 oz.");
    }

    #[test]
    fn review_chunks_overlap() {
        let body = (1..=600)
            .map(|n| format!("w{n}"))
            .collect::<Vec<_>>()
            .join(" ");
        let chunks = review_chunks(&body);
        // 600 / (256 - 32) = ceil(600/224) = 3
        assert_eq!(chunks.len(), 3);
        // First chunk is exactly 256 words.
        assert_eq!(chunks[0].split_whitespace().count(), 256);
    }

    #[test]
    fn review_chunks_short_input_one_chunk() {
        let body = "short review body";
        assert_eq!(review_chunks(body), vec!["short review body".to_string()]);
    }
}
