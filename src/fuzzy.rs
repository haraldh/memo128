use std::cmp::min;

use num_bigint::BigUint;
use num_traits::{ToPrimitive, Zero};
use sha2::{Digest, Sha256};

use crate::{
    Dictionary, Memo128, Memo128Error, ACTION_BITS, CHECKSUM_BITS, CHUNK_BITS, NUM_CHUNKS,
    OBJECT_BITS, OUTCOME_BITS, SETTING_BITS,
};

/// Type representing component indices for a sentence (character, setting, action, object, outcome)
type ComponentIndices = (usize, usize, usize, usize, usize);

/// Calculate the Levenshtein distance between two strings
///
/// The Levenshtein distance is a measure of the similarity between two strings.
/// It represents the minimum number of single-character edits (insertions, deletions,
/// or substitutions) required to change one string into the other.
pub fn levenshtein_distance(s1: &str, s2: &str) -> usize {
    // Handle empty strings
    if s1.is_empty() {
        return s2.len();
    }
    if s2.is_empty() {
        return s1.len();
    }

    let s1_chars: Vec<char> = s1.chars().collect();
    let s2_chars: Vec<char> = s2.chars().collect();
    let s1_len = s1_chars.len();
    let s2_len = s2_chars.len();

    // Dynamic programming matrix
    let mut dp = vec![vec![0; s2_len + 1]; s1_len + 1];

    // Initialize first row and column
    for (i, item) in dp.iter_mut().enumerate().take(s1_len + 1) {
        item[0] = i;
    }
    for j in 0..=s2_len {
        dp[0][j] = j;
    }

    // Fill the matrix
    for i in 1..=s1_len {
        for j in 1..=s2_len {
            let cost = if s1_chars[i - 1] == s2_chars[j - 1] {
                0
            } else {
                1
            };

            dp[i][j] = min(
                min(
                    dp[i - 1][j] + 1, // deletion
                    dp[i][j - 1] + 1, // insertion
                ),
                dp[i - 1][j - 1] + cost, // substitution
            );
        }
    }

    // Return the bottom-right value which contains the total distance
    dp[s1_len][s2_len]
}

/// Fuzzy matching decoder for the Memo128 system
///
/// This structure extends the functionality of Memo128 with fuzzy matching capabilities,
/// allowing decoding of imperfect sentences that might contain typos, minor rephrasing, etc.
pub struct FuzzyMemo128 {
    memo128: Memo128,
    max_levenshtein_distance: usize,
}

impl FuzzyMemo128 {
    /// Create a new FuzzyMemo128 instance with default dictionaries
    pub fn new(max_levenshtein_distance: usize) -> Result<Self, Memo128Error> {
        Ok(FuzzyMemo128 {
            memo128: Memo128::new()?,
            max_levenshtein_distance,
        })
    }

    /// Calculate 7-bit checksum from 128-bit data
    fn calculate_checksum(&self, data: &[u8]) -> u8 {
        let mut hasher = Sha256::new();
        hasher.update(data);
        let result = hasher.finalize();
        // Take the first 7 bits (MSB) of the hash
        (result[0] >> 1) & 0x7F
    }

    /// Find all possible matches within the given Levenshtein distance
    fn find_fuzzy_matches(
        &self,
        text: &str,
        dictionary: &Dictionary,
        max_distance: usize,
    ) -> Vec<(usize, usize)> {
        let mut matches = Vec::new();

        for (idx, entry) in dictionary.entries.iter().enumerate() {
            let distance = levenshtein_distance(text, entry);
            if distance <= max_distance {
                matches.push((idx, distance));
            }
        }

        // Sort matches by distance (closest first)
        matches.sort_by_key(|&(_, distance)| distance);
        matches
    }

    /// Parse a sentence with fuzzy matching to find all plausible component sequences
    ///
    /// This is the core fuzzy parsing algorithm that attempts to segment the sentence
    /// and match each segment against dictionary entries using Levenshtein distance.
    fn fuzzy_parse_sentence(&self, sentence: &str) -> Vec<ComponentIndices> {
        let mut results = Vec::new();

        // Access dictionaries through memo128 instance
        let character_dict = &self.memo128.get_character_dict();
        let setting_dict = &self.memo128.get_setting_dict();
        let action_dict = &self.memo128.get_action_dict();
        let object_dict = &self.memo128.get_object_dict();
        let outcome_dict = &self.memo128.get_outcome_dict();

        // Define a recursive helper function to find sequences
        fn find_sequences<'a>(
            sentence: &'a str,
            component_idx: usize,
            current_indices: &mut Vec<usize>,
            results: &mut Vec<ComponentIndices>,
            dictionaries: &[&&'a Dictionary],
            max_distance: usize,
            fuzzy_memo: &'a FuzzyMemo128,
        ) {
            // Base case: if we've matched all 5 components
            if component_idx == 5 {
                if sentence.trim().is_empty() {
                    // We have a complete match and used the entire input
                    results.push((
                        current_indices[0],
                        current_indices[1],
                        current_indices[2],
                        current_indices[3],
                        current_indices[4],
                    ));
                }
                return;
            }

            // Get the current dictionary
            let dictionary = dictionaries[component_idx];

            // Try different segmentation points
            // Start with more greedy segments to optimize for fewer branches
            let max_segment_len = if component_idx == 4 {
                // Last component (outcome) should match the rest of the sentence
                sentence.len()
            } else {
                // For other components, limit reasonable segment length
                // This helps reduce combinatorial explosion
                min(
                    sentence.len(),
                    100, // Reasonable max length for a component
                )
            };

            // We'll try segmenting at spaces first for a more efficient search
            // This is a heuristic to reduce the search space
            let space_positions: Vec<usize> = sentence
                .char_indices()
                .filter(|&(_, c)| c == ' ')
                .map(|(i, _)| i)
                .collect();

            // Add the end of string as a potential break point
            let mut segment_points = space_positions;
            if component_idx == 4 {
                segment_points = vec![sentence.len()]; // Last component takes all remaining text
            } else if !segment_points.is_empty() {
                segment_points.push(sentence.len());
                segment_points.sort();
            } else {
                // If no spaces found, use a more brute force approach
                segment_points = (1..=max_segment_len).collect();
            }

            for &k in &segment_points {
                if k > sentence.len() {
                    continue;
                }

                let prefix = &sentence[..k];

                // Get fuzzy matches for this prefix
                let matches = fuzzy_memo.find_fuzzy_matches(prefix, dictionary, max_distance);

                for (idx, _) in matches {
                    // Add this index to our current path
                    current_indices.push(idx);

                    // Continue with the rest of the sentence
                    let remainder = if k < sentence.len() {
                        // Skip the space after this component if it exists
                        if sentence[k..].starts_with(' ') {
                            &sentence[k + 1..]
                        } else {
                            &sentence[k..]
                        }
                    } else {
                        ""
                    };

                    // Recursively process the remainder
                    find_sequences(
                        remainder,
                        component_idx + 1,
                        current_indices,
                        results,
                        dictionaries,
                        max_distance,
                        fuzzy_memo,
                    );

                    // Remove this index before trying the next match
                    current_indices.pop();
                }
            }
        }

        // Set up the dictionary array to pass to our recursive function
        let dictionaries = [
            character_dict,
            setting_dict,
            action_dict,
            object_dict,
            outcome_dict,
        ];

        // Start the recursive search
        let mut current_indices = Vec::with_capacity(5);
        find_sequences(
            sentence,
            0,
            &mut current_indices,
            &mut results,
            &dictionaries[..],
            self.max_levenshtein_distance,
            self,
        );

        results
    }

    /// Decode imperfect sentences with fuzzy matching
    ///
    /// This function takes 3 potentially imperfect sentences and attempts to find
    /// all possible 128-bit payloads that could plausibly correspond to them and
    /// satisfy the checksum validation.
    pub fn fuzzy_decode(&self, input_sentences: &[String]) -> Result<Vec<String>, Memo128Error> {
        if input_sentences.len() != NUM_CHUNKS {
            return Err(Memo128Error::ParsingError(format!(
                "Expected exactly {} sentences, got {}",
                NUM_CHUNKS,
                input_sentences.len()
            )));
        }

        // Process each sentence to find all plausible component sequences
        let mut sentence_candidates: Vec<Vec<ComponentIndices>> = Vec::new();

        for sentence in input_sentences {
            let sentence = sentence.trim();
            let candidates = self.fuzzy_parse_sentence(sentence);

            if candidates.is_empty() {
                // If any sentence has no plausible parsing, we can't proceed
                return Err(Memo128Error::ParsingError(format!(
                    "No fuzzy matches found for sentence: {}",
                    sentence
                )));
            }

            sentence_candidates.push(candidates);
        }

        // Store valid hex results
        let mut valid_hex_results = Vec::new();

        // Generate all combinations and check each one
        self.check_candidates(
            &sentence_candidates,
            0,
            &mut Vec::with_capacity(NUM_CHUNKS),
            &mut valid_hex_results,
        );

        Ok(valid_hex_results)
    }

    /// Recursively check all combinations of component sequences
    fn check_candidates(
        &self,
        sentence_candidates: &[Vec<ComponentIndices>],
        sentence_idx: usize,
        current_combo: &mut Vec<ComponentIndices>,
        valid_hex_results: &mut Vec<String>,
    ) {
        // Base case: we've assembled a complete combination of component sequences
        if sentence_idx == sentence_candidates.len() {
            // We have a complete combination, check if it's valid
            let reconstructed_135_num = self.reconstruct_number(current_combo);

            // Separate data and checksum
            let checksum_mask = BigUint::from((1u16 << CHECKSUM_BITS) - 1);
            let checksum_bits_decoded = (&reconstructed_135_num & &checksum_mask).to_u8().unwrap();
            let data_num_decoded = &reconstructed_135_num >> CHECKSUM_BITS;

            // Convert data_num_decoded to bytes
            let data_bytes_decoded = data_num_decoded.to_bytes_be();

            // Pad with zeros if necessary
            let mut padded_bytes = vec![0; 16];
            let offset = 16 - data_bytes_decoded.len();
            padded_bytes[offset..].copy_from_slice(&data_bytes_decoded);

            // Calculate and verify checksum
            let checksum_bits_calculated = self.calculate_checksum(&padded_bytes);

            if checksum_bits_decoded == checksum_bits_calculated {
                // Valid match! Convert to hex and add to results
                let hex_result = Memo128::bytes_to_hex(&padded_bytes);

                // Only add if it's not already in the results
                if !valid_hex_results.contains(&hex_result) {
                    valid_hex_results.push(hex_result);
                }
            }

            return;
        }

        // Recursive case: try each candidate for the current sentence
        for &candidate in &sentence_candidates[sentence_idx] {
            current_combo.push(candidate);
            self.check_candidates(
                sentence_candidates,
                sentence_idx + 1,
                current_combo,
                valid_hex_results,
            );
            current_combo.pop();
        }
    }

    /// Reconstruct the 135-bit number from component indices
    fn reconstruct_number(
        &self,
        component_combos: &[ComponentIndices],
    ) -> BigUint {
        let mut reconstructed_135_num = BigUint::zero();

        for &(idx_c, idx_s, idx_a, idx_o, idx_k) in component_combos {
            // Reconstruct chunk value
            let chunk_value = (BigUint::from(idx_c)
                << (SETTING_BITS + ACTION_BITS + OBJECT_BITS + OUTCOME_BITS))
                | (BigUint::from(idx_s) << (ACTION_BITS + OBJECT_BITS + OUTCOME_BITS))
                | (BigUint::from(idx_a) << (OBJECT_BITS + OUTCOME_BITS))
                | (BigUint::from(idx_o) << OUTCOME_BITS)
                | BigUint::from(idx_k);

            // Append to the reconstructed number
            reconstructed_135_num = (reconstructed_135_num << CHUNK_BITS) | chunk_value;
        }

        reconstructed_135_num
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_levenshtein_distance() {
        // Test cases for Levenshtein distance
        assert_eq!(levenshtein_distance("kitten", "kitten"), 0);
        assert_eq!(levenshtein_distance("kitten", "sitting"), 3);
        assert_eq!(levenshtein_distance("", "abc"), 3);
        assert_eq!(levenshtein_distance("abc", ""), 3);
        assert_eq!(levenshtein_distance("", ""), 0);
        assert_eq!(levenshtein_distance("a", "b"), 1);
        assert_eq!(levenshtein_distance("ab", "abc"), 1);
        assert_eq!(levenshtein_distance("abc", "abcd"), 1);
        assert_eq!(levenshtein_distance("abcd", "abc"), 1);
    }
}
