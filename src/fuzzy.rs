//! Fuzzy decoding module for Memo128
//!
//! This module extends Memo128 with fuzzy matching capabilities, allowing for the
//! decoding of imperfect or misremembered sentences. It uses the Levenshtein distance
//! algorithm to match phrases that might contain typos, minor rephrasing, or word form
//! changes.
//!
//! ## Features
//!
//! - Levenshtein distance calculation to measure string similarity
//! - Configurable fuzzy matching threshold
//! - Multiple potential matches when ambiguities exist
//! - Sentence segmentation and component matching
//! - Checksum validation to ensure valid matches
//!
//! ## Usage
//!
//! ```rust,no_run
//! # // Note: This example requires dictionary files to be present, so we use no_run
//! # use memo128::fuzzy::FuzzyMemo128;
//! # fn example() -> Result<(), Box<dyn std::error::Error>> {
//! // Create a fuzzy decoder with maximum Levenshtein distance of 2
//! let fuzzy_decoder = FuzzyMemo128::new(2)?;
//!
//! // Decode sentences with imperfections (typos, rephrasing, etc.)
//! let imperfect_sentences = vec![
//!     "a brave mouze inside a cosmic cathedral disconnected a question of time but it was too late".to_string(),
//!     "a worried parent within a cosmic algorithm accepted a math impossibility as code predicted".to_string(),
//!     "the fjord elder inside the particle accelerator stole a reality glitch rebooting systems".to_string(),
//! ];
//!
//! // Get all possible valid matches
//! let possible_matches = fuzzy_decoder.fuzzy_decode(&imperfect_sentences)?;
//! # Ok(())
//! # }
//! ```

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
///
/// # Arguments
///
/// * `s1` - First string to compare
/// * `s2` - Second string to compare
///
/// # Returns
///
/// The Levenshtein distance between the two strings
///
/// # Examples
///
/// ```
/// use memo128::fuzzy::levenshtein_distance;
///
/// // Identical strings have distance 0
/// assert_eq!(levenshtein_distance("hello", "hello"), 0);
///
/// // Changing one character is distance 1
/// assert_eq!(levenshtein_distance("hello", "hallo"), 1);
///
/// // Adding/removing one character is distance 1
/// assert_eq!(levenshtein_distance("hello", "hell"), 1);
/// assert_eq!(levenshtein_distance("hello", "helloo"), 1);
/// ```
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
/// It uses the Levenshtein distance algorithm to match sentence components with dictionary
/// entries.
pub struct FuzzyMemo128 {
    /// Reference to the standard Memo128 instance with dictionaries
    memo128: Memo128,

    /// Maximum allowed Levenshtein distance for fuzzy matching
    /// Higher values allow more flexible matching but may increase computation time
    max_levenshtein_distance: usize,
}

impl FuzzyMemo128 {
    /// Create a new FuzzyMemo128 instance with default dictionaries
    ///
    /// # Arguments
    ///
    /// * `max_levenshtein_distance` - Maximum Levenshtein distance to allow when matching phrases.
    ///   - 0: Only exact matches (equivalent to standard Memo128)
    ///   - 1: Allow minor typos like "mouze" instead of "mouse"
    ///   - 2: Allow word form changes like "systems" vs "system"
    ///   - 3+: Allow more extensive rephrasing, but may be slow and less accurate
    ///
    /// # Returns
    ///
    /// Result containing either the initialized FuzzyMemo128 instance or a Memo128Error
    ///
    /// # Errors
    ///
    /// Returns an error if the underlying Memo128 instance fails to initialize
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # // Note: This example requires dictionary files to be present, so we use no_run
    /// # use memo128::fuzzy::FuzzyMemo128;
    /// # fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// // Create a fuzzy decoder with max Levenshtein distance of 2
    /// let fuzzy_decoder = FuzzyMemo128::new(2)?;
    /// # Ok(())
    /// # }
    /// ```
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
    /// 
    /// # Arguments
    ///
    /// * `text` - The text segment to find matches for
    /// * `dictionary` - The dictionary to search in
    /// * `max_distance` - Maximum allowed Levenshtein distance
    /// 
    /// # Returns
    ///
    /// A vector of tuples containing (dictionary index, Levenshtein distance, entry length)
    /// The entries are sorted by distance (closest first) and then by length (longer entries first)
    fn find_fuzzy_matches(
        &self,
        text: &str,
        dictionary: &Dictionary,
        max_distance: usize,
    ) -> Vec<(usize, usize, usize)> {
        let mut matches = Vec::new();

        // Calculate the minimum length of dictionary entries to consider
        // This optimization helps avoid checking entries that are too short
        let min_len = if text.len() > max_distance {
            text.len() - max_distance
        } else {
            0
        };

        // Calculate the maximum length of dictionary entries to consider
        // This optimization helps avoid checking entries that are too long
        let max_len = text.len() + max_distance;

        for (idx, entry) in dictionary.entries.iter().enumerate() {
            // Skip entries that are too short or too long (quick filter)
            if entry.len() < min_len || entry.len() > max_len {
                continue;
            }

            let distance = levenshtein_distance(text, entry);
            if distance <= max_distance {
                matches.push((idx, distance, entry.len()));
            }
        }

        // Sort matches by distance (closest first) and then by length (longer entries first)
        // This prioritizes both close matches and longer phrases which are usually more specific
        matches.sort_by(|&(_, dist_a, len_a), &(_, dist_b, len_b)| {
            // First compare by distance
            match dist_a.cmp(&dist_b) {
                std::cmp::Ordering::Equal => {
                    // If distances are equal, prefer longer matches
                    len_b.cmp(&len_a) // Reversed to put longer entries first
                }
                ordering => ordering,
            }
        });
        
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

            // Determine how to segment the sentence based on component position
            let max_segment_len = if component_idx == 4 {
                // Last component (outcome) should match the rest of the sentence
                sentence.len()
            } else {
                // For other components, limit reasonable segment length
                // This helps reduce combinatorial explosion
                min(
                    sentence.len(),
                    // Use different max lengths based on dictionary expected entry size
                    match component_idx {
                        0 => 50, // Character (more likely to be longer)
                        1 => 50, // Setting (can be longer phrase)
                        2 => 30, // Action (usually shorter)
                        3 => 40, // Object (medium length)
                        _ => 100, // Fallback (shouldn't happen)
                    },
                )
            };

            // Enhanced segmentation strategy:
            // 1. Try full sentence segments for last component
            // 2. Try breaking at spaces for natural word boundaries
            // 3. Try word sequences of varying lengths for better phrase detection
            let mut segment_points = Vec::new();
            
            if component_idx == 4 {
                // Last component - use all remaining text
                segment_points.push(sentence.len());
            } else {
                // Collect all space positions
                let space_positions: Vec<usize> = sentence
                    .char_indices()
                    .filter(|&(_, c)| c == ' ')
                    .map(|(i, _)| i)
                    .collect();
                
                // For components other than last one, try different segmentation strategies
                if !space_positions.is_empty() {
                    // Try segmenting at different word boundaries
                    // 1 word, 2 words, 3 words, etc. 
                    for i in 0..min(5, space_positions.len()) {
                        segment_points.push(space_positions[i]);
                    }
                    
                    // Also add some longer segments to try
                    if space_positions.len() >= 2 {
                        for i in (2..min(10, space_positions.len())).step_by(2) {
                            segment_points.push(space_positions[i]);
                        }
                    }
                    
                    // Also add full sentence if it's within reasonable length
                    if sentence.len() <= max_segment_len {
                        segment_points.push(sentence.len());
                    }
                    
                    // Sort and deduplicate
                    segment_points.sort();
                    segment_points.dedup();
                } else {
                    // If no spaces found, use a more strategic approach to limit combinatorial explosion
                    // Try reasonable points instead of every position
                    if sentence.len() <= 10 {
                        // For very short sequences, try every position
                        segment_points = (1..=min(max_segment_len, sentence.len())).collect();
                    } else {
                        // For longer sequences, try at regular intervals to reduce combinations
                        for len in (5..=min(max_segment_len, sentence.len())).step_by(5) {
                            segment_points.push(len);
                        }
                    }
                }
            }

            // Limit total number of segment points to prevent excessive recursion
            if segment_points.len() > 15 {
                let mut limited_points = Vec::with_capacity(15);
                let step = segment_points.len() / 15;
                for i in (0..segment_points.len()).step_by(step.max(1)) {
                    limited_points.push(segment_points[i]);
                }
                // Always include the last segment point
                if let Some(&last) = segment_points.last() {
                    if limited_points.last() != Some(&last) {
                        limited_points.push(last);
                    }
                }
                segment_points = limited_points;
            }

            for &k in &segment_points {
                if k == 0 || k > sentence.len() {
                    continue;
                }

                let prefix = &sentence[..k];

                // Get fuzzy matches for this prefix
                let matches = fuzzy_memo.find_fuzzy_matches(prefix, dictionary, max_distance);

                // Consider no more than 5 best matches per segment to limit recursion
                for (idx, _, _) in matches.into_iter().take(5) {
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
                    
                    // If we already have a significant number of results, stop adding more
                    // This prevents excessive recursion while still finding good matches
                    if results.len() >= 50 {
                        return;
                    }
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
    /// satisfy the checksum validation. Unlike the standard `decode` method in `Memo128`,
    /// this function can handle typos, minor rephrasing, and word form changes.
    ///
    /// # Arguments
    ///
    /// * `input_sentences` - A slice of three strings containing the sentences to decode
    ///
    /// # Returns
    ///
    /// Result containing either a Vec of possible 32-character hex strings or a Memo128Error
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The number of sentences is not exactly 3
    /// - No plausible fuzzy matches can be found for any of the sentences
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// # // Note: This example requires dictionary files to be present, so we use no_run
    /// # use memo128::fuzzy::FuzzyMemo128;
    /// # fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let fuzzy_decoder = FuzzyMemo128::new(2)?;
    ///
    /// // Note the typos and word form changes compared to the original:
    /// // "mouze" instead of "mouse", "within a cosmic" instead of "within the cosmic",
    /// // "rebooting systems" instead of "rebooting the system"
    /// let imperfect_sentences = vec![
    ///     "a brave mouze inside a cosmic cathedral disconnected a question of time but it was already too late".to_string(),
    ///     "a worried parent within a cosmic algorithm accepted a mathematical impossibility as code predicted".to_string(),
    ///     "the fjord elder inside a particle accelerator stole a reality glitch rebooting systems".to_string(),
    /// ];
    ///
    /// let possible_hex_values = fuzzy_decoder.fuzzy_decode(&imperfect_sentences)?;
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// # Notes
    ///
    /// - The function may return multiple possible matches if the imperfect sentences
    ///   can be interpreted in different ways that satisfy the checksum
    /// - Higher `max_levenshtein_distance` values will allow more flexibility but may
    ///   increase computation time and possibly return false positives
    pub fn fuzzy_decode(&self, input_sentences: &[String]) -> Result<Vec<String>, Memo128Error> {
        // Validate we have exactly 3 sentences
        if input_sentences.len() != NUM_CHUNKS {
            return Err(Memo128Error::ParsingError(format!(
                "Expected exactly {} sentences, got {}",
                NUM_CHUNKS,
                input_sentences.len()
            )));
        }

        // Normalize input sentences - trim whitespace and normalize spaces
        let normalized_sentences: Vec<String> = input_sentences
            .iter()
            .map(|s| {
                let trimmed = s.trim();
                // Replace multiple consecutive spaces with a single space
                let mut normalized = String::with_capacity(trimmed.len());
                let mut last_was_space = false;
                
                for c in trimmed.chars() {
                    if c.is_whitespace() {
                        if !last_was_space {
                            normalized.push(' ');
                            last_was_space = true;
                        }
                    } else {
                        normalized.push(c);
                        last_was_space = false;
                    }
                }
                
                normalized
            })
            .collect();

        // Process each sentence to find all plausible component sequences
        let mut sentence_candidates: Vec<Vec<ComponentIndices>> = Vec::new();

        for (i, sentence) in normalized_sentences.iter().enumerate() {
            if sentence.is_empty() {
                return Err(Memo128Error::ParsingError(format!(
                    "Sentence {} is empty after normalization", i + 1
                )));
            }
            
            let candidates = self.fuzzy_parse_sentence(sentence);

            if candidates.is_empty() {
                // If any sentence has no plausible parsing, we can't proceed
                return Err(Memo128Error::ParsingError(format!(
                    "No fuzzy matches found for sentence {}: {}",
                    i + 1, sentence
                )));
            }

            // Limit the number of candidates per sentence to prevent combinatorial explosion
            let max_candidates = 50;
            let limited_candidates = if candidates.len() > max_candidates {
                candidates[0..max_candidates].to_vec()
            } else {
                candidates
            };
            
            sentence_candidates.push(limited_candidates);
        }

        // Store valid hex results
        let mut valid_hex_results = Vec::new();

        // Calculate the total number of combinations
        let total_combinations: usize = sentence_candidates.iter()
            .map(|candidates| candidates.len())
            .product();
            
        // If we have too many combinations, return an error to prevent excessive computation
        if total_combinations > 1_000_000 {
            return Err(Memo128Error::ParsingError(format!(
                "Too many possible combinations ({}) to check efficiently. Try increasing the fuzzy matching threshold.",
                total_combinations
            )));
        }

        // Generate all combinations and check each one
        self.check_candidates(
            &sentence_candidates,
            0,
            &mut Vec::with_capacity(NUM_CHUNKS),
            &mut valid_hex_results,
        );

        // Sort results to ensure stable output across runs
        valid_hex_results.sort();
        
        // Limit the number of results if we have too many
        let max_results = 100;
        if valid_hex_results.len() > max_results {
            valid_hex_results.truncate(max_results);
        }

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
    fn reconstruct_number(&self, component_combos: &[ComponentIndices]) -> BigUint {
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
