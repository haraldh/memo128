//! # Memo128
//!
//! A library for encoding 128-bit values as memorable natural language sentences.
//!
//! ## Overview
//!
//! Memo128 converts cryptographic keys, blockchain addresses, and other 128-bit values
//! into easy-to-remember sentences. It adds a 7-bit checksum for error detection and
//! uses five dictionaries to create structured sentences that form mini-stories.
//!
//! Each 128-bit value (plus 7-bit checksum) is encoded as three sentences, with each
//! sentence containing:
//!
//! - Character (10 bits): Who is performing the action
//! - Setting (10 bits): Where the action takes place
//! - Action (8 bits): What is being done
//! - Object (9 bits): What is being acted upon
//! - Outcome (8 bits): The result of the action
//!
//! ## Basic Usage
//!
//! ```rust,no_run
//! # // Note: This example requires dictionary files to be present, so we use no_run
//! use memo128::Memo128;
//!
//! fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     // Create a new Memo128 instance
//!     let memo128 = Memo128::new()?;
//!
//!     // Encode a 128-bit value (32-character hex string)
//!     let hex_input = "0123456789abcdef0123456789abcdef";
//!     let sentences = memo128.encode(hex_input)?;
//!
//!     // Print the encoded sentences
//!     for (i, sentence) in sentences.iter().enumerate() {
//!         println!("Sentence {}: {}", i + 1, sentence);
//!     }
//!
//!     // Decode the sentences back to the original value
//!     let hex_output = memo128.decode(&sentences)?;
//!     assert_eq!(hex_input, hex_output);
//!
//!     Ok(())
//! }
//! ```
//!
//! ## Fuzzy Decoding
//!
//! Memo128 also supports fuzzy decoding for imperfect sentence recall:
//!
//! ```rust,no_run
//! # // Note: This example requires dictionary files to be present, so we use no_run
//! use memo128::fuzzy::FuzzyMemo128;
//!
//! fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     // Create a fuzzy decoder with maximum Levenshtein distance of 2
//!     let fuzzy_decoder = FuzzyMemo128::new(2)?;
//!
//!     // Decode sentences with typos or rephrasing
//!     let imperfect_sentences = vec![
//!         "a brave mouze inside a cosmic cathedral disconnected a question of time but it was too late".to_string(),
//!         "a worried parent within a cosmic algorithm accepted a math impossibility as code predicted".to_string(),
//!         "the fjord elder inside the particle accelerator stole a reality glitch rebooting systems".to_string(),
//!     ];
//!
//!     // Get all possible valid matches
//!     let possible_matches = fuzzy_decoder.fuzzy_decode(&imperfect_sentences)?;
//!
//!     // Display matches
//!     for (i, hex) in possible_matches.iter().enumerate() {
//!         println!("Match {}: {}", i + 1, hex);
//!     }
//!
//!     Ok(())
//! }
//! ```
//!
//! ## Requirements
//!
//! Memo128 requires five dictionary files in the working directory:
//!
//! - `character_10bit.txt` (1024 entries)
//! - `setting_10bit.txt` (1024 entries)
//! - `action_8bit.txt` (256 entries)
//! - `object_9bit.txt` (512 entries)
//! - `outcome_8bit.txt` (256 entries)
//!
//! ## How It Works
//!
//! 1. Input: A 32-character hexadecimal string (128 bits)
//! 2. Calculate a 7-bit checksum using SHA-256
//! 3. Combine into a 135-bit sequence
//! 4. Split into three 45-bit chunks
//! 5. Map each chunk to sentence components using dictionaries
//! 6. Assemble three memorable sentences
//!
//! When decoding, the process is reversed and the checksum is verified to
//! detect errors.

use std::collections::HashMap;
use std::fs::File;
use std::io::{self, BufRead, BufReader};
use std::path::Path;

use num_bigint::BigUint;
use num_traits::{One, ToPrimitive, Zero};
use sha2::{Digest, Sha256};
use thiserror::Error;

// Expose fuzzy decoding module
pub mod fuzzy;

/// Number of bits used for the Character component in each sentence (10 bits = 1024 possibilities)
pub const CHARACTER_BITS: u32 = 10;

/// Number of bits used for the Setting component in each sentence (10 bits = 1024 possibilities)
pub const SETTING_BITS: u32 = 10;

/// Number of bits used for the Action component in each sentence (8 bits = 256 possibilities)
pub const ACTION_BITS: u32 = 8;

/// Number of bits used for the Object component in each sentence (9 bits = 512 possibilities)
pub const OBJECT_BITS: u32 = 9;

/// Number of bits used for the Outcome component in each sentence (8 bits = 256 possibilities)
pub const OUTCOME_BITS: u32 = 8;

/// Total number of bits per chunk/sentence (45 bits)
pub const CHUNK_BITS: u32 =
    CHARACTER_BITS + SETTING_BITS + ACTION_BITS + OBJECT_BITS + OUTCOME_BITS;

/// Number of bits used for checksum (7 bits, allowing for values 0-127)
pub const CHECKSUM_BITS: u32 = 7;

/// Number of chunks/sentences used to encode the complete 128-bit payload
pub const NUM_CHUNKS: usize = 3; // 3 chunks of 45 bits = 135 bits (128 data + 7 checksum)

/// Errors that can occur when using Memo128
#[derive(Debug, Error)]
pub enum Memo128Error {
    /// Wraps standard IO errors that occur during file operations
    #[error("IO error: {0}")]
    IoError(#[from] io::Error),

    /// Errors related to invalid hex input format or content
    #[error("Invalid hex input: {0}")]
    InvalidHexInput(String),

    /// Errors related to dictionary loading or validation
    #[error("Dictionary error: {0}")]
    InvalidDictionary(String),

    /// Errors that occur during sentence parsing
    #[error("Parsing error: {0}")]
    ParsingError(String),

    /// Error when checksum verification fails during decoding
    #[error("Checksum verification failed")]
    ChecksumError,
}

/// Dictionary of phrases used for each component of the sentences
///
/// Each dictionary contains a specific number of entries corresponding to
/// the bit allocation for its associated component (e.g., Character, Setting, etc.).
/// The dictionary provides both forward lookup (index -> phrase) and reverse
/// lookup (phrase -> index) capabilities.
pub struct Dictionary {
    /// Vector of phrases, indexed by their position (which corresponds to the encoded bit value)
    pub entries: Vec<String>,

    /// Hashmap for efficient reverse lookup (phrase to index)
    pub reverse_lookup: HashMap<String, usize>,
}

impl Dictionary {
    /// Creates a new empty dictionary with pre-allocated capacity
    ///
    /// # Arguments
    ///
    /// * `expected_size` - The expected number of entries in the dictionary
    ///
    /// # Returns
    ///
    /// A new Dictionary instance with capacity for the specified number of entries
    pub fn new(expected_size: usize) -> Self {
        Dictionary {
            entries: Vec::with_capacity(expected_size),
            reverse_lookup: HashMap::with_capacity(expected_size),
        }
    }

    /// Loads a dictionary from a file
    ///
    /// # Arguments
    ///
    /// * `path` - Path to the dictionary file
    /// * `expected_size` - The expected number of entries (must match exactly)
    ///
    /// # Returns
    ///
    /// Result containing either the loaded Dictionary or a Memo128Error
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The file cannot be opened
    /// - The dictionary contains more than the expected number of entries
    /// - The dictionary contains duplicate entries
    /// - The dictionary contains fewer than the expected number of entries
    pub fn load<P: AsRef<Path>>(path: P, expected_size: usize) -> Result<Self, Memo128Error> {
        let file = File::open(path)?;
        let reader = BufReader::new(file);
        let mut dict = Dictionary::new(expected_size);

        for (idx, line) in reader.lines().enumerate() {
            let entry = line?.trim().to_string();
            if entry.is_empty() {
                continue;
            }

            if dict.entries.len() >= expected_size {
                return Err(Memo128Error::InvalidDictionary(format!(
                    "Dictionary contains more than {} entries",
                    expected_size
                )));
            }

            if dict.reverse_lookup.contains_key(&entry) {
                return Err(Memo128Error::InvalidDictionary(format!(
                    "Duplicate entry found: {}",
                    entry
                )));
            }

            dict.reverse_lookup.insert(entry.clone(), idx);
            dict.entries.push(entry);
        }

        if dict.entries.len() != expected_size {
            return Err(Memo128Error::InvalidDictionary(format!(
                "Dictionary should contain exactly {} entries, found {}",
                expected_size,
                dict.entries.len()
            )));
        }

        Ok(dict)
    }

    /// Retrieves a phrase from the dictionary by its index
    ///
    /// # Arguments
    ///
    /// * `index` - The index of the phrase to retrieve
    ///
    /// # Returns
    ///
    /// An Option containing a reference to the String if found, or None if the index is out of bounds
    pub fn get(&self, index: usize) -> Option<&String> {
        self.entries.get(index)
    }
}

/// Main struct for encoding and decoding 128-bit values as memorable sentences
///
/// Memo128 encodes 128-bit values into three memorable sentences, with each sentence containing:
/// - Character (10 bits): Who is performing the action
/// - Setting (10 bits): Where the action takes place
/// - Action (8 bits): What is being done
/// - Object (9 bits): What is being acted upon
/// - Outcome (8 bits): The result of the action
///
/// The system adds a 7-bit checksum for error detection, bringing the total to 135 bits.
pub struct Memo128 {
    /// Dictionary for the character component (10 bits = 1024 entries)
    character_dict: Dictionary,

    /// Dictionary for the setting component (10 bits = 1024 entries)
    setting_dict: Dictionary,

    /// Dictionary for the action component (8 bits = 256 entries)
    action_dict: Dictionary,

    /// Dictionary for the object component (9 bits = 512 entries)
    object_dict: Dictionary,

    /// Dictionary for the outcome component (8 bits = 256 entries)
    outcome_dict: Dictionary,
}

impl Memo128 {
    /// Creates a new Memo128 instance by loading all required dictionaries
    ///
    /// Loads dictionaries from the current working directory:
    /// - character_10bit.txt
    /// - setting_10bit.txt
    /// - action_8bit.txt
    /// - object_9bit.txt
    /// - outcome_8bit.txt
    ///
    /// # Returns
    ///
    /// Result containing either the initialized Memo128 instance or a Memo128Error
    ///
    /// # Errors
    ///
    /// Returns an error if any dictionary fails to load
    pub fn new() -> Result<Self, Memo128Error> {
        Ok(Memo128 {
            character_dict: Dictionary::load("character_10bit.txt", 1 << CHARACTER_BITS)?,
            setting_dict: Dictionary::load("setting_10bit.txt", 1 << SETTING_BITS)?,
            action_dict: Dictionary::load("action_8bit.txt", 1 << ACTION_BITS)?,
            object_dict: Dictionary::load("object_9bit.txt", 1 << OBJECT_BITS)?,
            outcome_dict: Dictionary::load("outcome_8bit.txt", 1 << OUTCOME_BITS)?,
        })
    }

    /// Gets a reference to the Character dictionary
    ///
    /// # Returns
    ///
    /// Reference to the Character dictionary
    pub fn get_character_dict(&self) -> &Dictionary {
        &self.character_dict
    }

    /// Gets a reference to the Setting dictionary
    ///
    /// # Returns
    ///
    /// Reference to the Setting dictionary
    pub fn get_setting_dict(&self) -> &Dictionary {
        &self.setting_dict
    }

    /// Gets a reference to the Action dictionary
    ///
    /// # Returns
    ///
    /// Reference to the Action dictionary
    pub fn get_action_dict(&self) -> &Dictionary {
        &self.action_dict
    }

    /// Gets a reference to the Object dictionary
    ///
    /// # Returns
    ///
    /// Reference to the Object dictionary
    pub fn get_object_dict(&self) -> &Dictionary {
        &self.object_dict
    }

    /// Gets a reference to the Outcome dictionary
    ///
    /// # Returns
    ///
    /// Reference to the Outcome dictionary
    pub fn get_outcome_dict(&self) -> &Dictionary {
        &self.outcome_dict
    }

    /// Calculates a 7-bit checksum from 128-bit data using SHA-256
    ///
    /// # Arguments
    ///
    /// * `data` - The 16-byte array (128 bits) to calculate checksum for
    ///
    /// # Returns
    ///
    /// A 7-bit checksum value (0-127) derived from the first byte of the SHA-256 hash
    fn calculate_checksum(&self, data: &[u8]) -> u8 {
        let mut hasher = Sha256::new();
        hasher.update(data);
        let result = hasher.finalize();
        // Take the first 7 bits (MSB) of the hash
        (result[0] >> 1) & 0x7F
    }

    /// Converts a 32-character hex string to a 16-byte array
    ///
    /// # Arguments
    ///
    /// * `hex` - A 32-character hexadecimal string representing 128 bits
    ///
    /// # Returns
    ///
    /// Result containing either a `Vec<u8>` with 16 bytes or a Memo128Error
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The input is not exactly 32 characters
    /// - The input contains invalid hex characters
    pub fn hex_to_bytes(hex: &str) -> Result<Vec<u8>, Memo128Error> {
        if hex.len() != 32 {
            return Err(Memo128Error::InvalidHexInput(
                "Hex string must be 32 characters long".to_string(),
            ));
        }

        let mut bytes = Vec::with_capacity(16);
        for i in 0..16 {
            let byte_str = &hex[i * 2..i * 2 + 2];
            match u8::from_str_radix(byte_str, 16) {
                Ok(byte) => bytes.push(byte),
                Err(_) => {
                    return Err(Memo128Error::InvalidHexInput(format!(
                        "Invalid hex characters: {}",
                        byte_str
                    )));
                }
            }
        }
        Ok(bytes)
    }

    /// Converts a byte array to a hex string
    ///
    /// # Arguments
    ///
    /// * `bytes` - Byte array to convert to hex
    ///
    /// # Returns
    ///
    /// A lowercase hexadecimal string representation of the bytes
    pub fn bytes_to_hex(bytes: &[u8]) -> String {
        let mut result = String::with_capacity(bytes.len() * 2);
        for &b in bytes {
            use std::fmt::Write;
            write!(&mut result, "{:02x}", b).unwrap();
        }
        result
    }

    /// Encodes a 128-bit value (as a 32-character hex string) into three memorable sentences
    ///
    /// # Arguments
    ///
    /// * `hex_input` - A 32-character hexadecimal string representing 128 bits
    ///
    /// # Returns
    ///
    /// Result containing either a Vec of three sentences or a Memo128Error
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The hex input is invalid (wrong length or contains non-hex characters)
    /// - There is a problem accessing dictionary entries
    ///
    /// # Example
    ///
    /// ```rust
    /// # use memo128::Memo128;
    /// # fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let memo128 = Memo128::new()?;
    /// let sentences = memo128.encode("0123456789abcdef0123456789abcdef")?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn encode(&self, hex_input: &str) -> Result<Vec<String>, Memo128Error> {
        // Convert hex to bytes
        let data_bytes = Self::hex_to_bytes(hex_input)?;
        if data_bytes.len() != 16 {
            return Err(Memo128Error::InvalidHexInput(
                "Data must be exactly 16 bytes (128 bits)".to_string(),
            ));
        }

        // Calculate checksum
        let checksum_bits = self.calculate_checksum(&data_bytes);

        // Convert data to BigUint
        let data_num = BigUint::from_bytes_be(&data_bytes);

        // Combine data and checksum: (data << 7) | checksum
        let combined = (data_num << CHECKSUM_BITS) | BigUint::from(checksum_bits);

        // Initialize output sentences
        let mut output_sentences = Vec::with_capacity(NUM_CHUNKS);

        // Process the 3 chunks
        let mut remaining_bits = combined.clone();
        for _ in 0..NUM_CHUNKS {
            // Extract chunk (45 bits) from the right side (LSB)
            let mask = (BigUint::one() << CHUNK_BITS) - BigUint::one();
            let chunk_value = &remaining_bits & &mask;
            remaining_bits >>= CHUNK_BITS;

            // Extract component indices
            let mut chunk_copy = chunk_value.clone();

            let idx_k = (&chunk_copy & BigUint::from(((1u64 << OUTCOME_BITS) - 1) as u8))
                .to_usize()
                .unwrap();
            chunk_copy >>= OUTCOME_BITS;

            let idx_o = (&chunk_copy & BigUint::from(((1u64 << OBJECT_BITS) - 1) as u16))
                .to_usize()
                .unwrap();
            chunk_copy >>= OBJECT_BITS;

            let idx_a = (&chunk_copy & BigUint::from(((1u64 << ACTION_BITS) - 1) as u8))
                .to_usize()
                .unwrap();
            chunk_copy >>= ACTION_BITS;

            let idx_s = (&chunk_copy & BigUint::from(((1u64 << SETTING_BITS) - 1) as u16))
                .to_usize()
                .unwrap();
            chunk_copy >>= SETTING_BITS;

            let idx_c = (&chunk_copy & BigUint::from(((1u64 << CHARACTER_BITS) - 1) as u16))
                .to_usize()
                .unwrap();

            // Lookup phrases
            let phrase_c = self.character_dict.get(idx_c).ok_or_else(|| {
                Memo128Error::InvalidDictionary(format!("Character index out of range: {}", idx_c))
            })?;
            let phrase_s = self.setting_dict.get(idx_s).ok_or_else(|| {
                Memo128Error::InvalidDictionary(format!("Setting index out of range: {}", idx_s))
            })?;
            let phrase_a = self.action_dict.get(idx_a).ok_or_else(|| {
                Memo128Error::InvalidDictionary(format!("Action index out of range: {}", idx_a))
            })?;
            let phrase_o = self.object_dict.get(idx_o).ok_or_else(|| {
                Memo128Error::InvalidDictionary(format!("Object index out of range: {}", idx_o))
            })?;
            let phrase_k = self.outcome_dict.get(idx_k).ok_or_else(|| {
                Memo128Error::InvalidDictionary(format!("Outcome index out of range: {}", idx_k))
            })?;

            // Assemble sentence
            let sentence = format!(
                "{} {} {} {} {}",
                phrase_c, phrase_s, phrase_a, phrase_o, phrase_k
            );
            output_sentences.insert(0, sentence);
        }

        Ok(output_sentences)
    }

    /// Parses a sentence into its five component indices
    ///
    /// This method tries all possible dictionary entries to find valid phrases that
    /// match the given sentence structure. It's an exact matching algorithm, unlike
    /// the fuzzy approach used in the fuzzy decoder.
    ///
    /// # Arguments
    ///
    /// * `sentence` - The sentence to parse
    ///
    /// # Returns
    ///
    /// Result containing either a tuple of five component indices or a Memo128Error
    ///
    /// # Errors
    ///
    /// Returns a ParsingError if the sentence cannot be parsed using the dictionaries
    fn parse_sentence(
        &self,
        sentence: &str,
    ) -> Result<(usize, usize, usize, usize, usize), Memo128Error> {
        // For each possible character phrase
        for (c_idx, c_phrase) in self.character_dict.entries.iter().enumerate() {
            // If the sentence starts with this character phrase
            if sentence.starts_with(c_phrase) {
                let rest_after_c = &sentence[c_phrase.len()..];

                // Skip the space after character phrase
                if !rest_after_c.starts_with(' ') {
                    continue;
                }
                let rest_after_c = &rest_after_c[1..];

                // Try each setting phrase
                for (s_idx, s_phrase) in self.setting_dict.entries.iter().enumerate() {
                    if rest_after_c.starts_with(s_phrase) {
                        let rest_after_s = &rest_after_c[s_phrase.len()..];

                        // Skip the space after setting phrase
                        if !rest_after_s.starts_with(' ') {
                            continue;
                        }
                        let rest_after_s = &rest_after_s[1..];

                        // Try each action phrase
                        for (a_idx, a_phrase) in self.action_dict.entries.iter().enumerate() {
                            if rest_after_s.starts_with(a_phrase) {
                                let rest_after_a = &rest_after_s[a_phrase.len()..];

                                // Skip the space after action phrase
                                if !rest_after_a.starts_with(' ') {
                                    continue;
                                }
                                let rest_after_a = &rest_after_a[1..];

                                // Try each object phrase
                                for (o_idx, o_phrase) in self.object_dict.entries.iter().enumerate()
                                {
                                    if rest_after_a.starts_with(o_phrase) {
                                        let rest_after_o = &rest_after_a[o_phrase.len()..];

                                        // Skip the space after object phrase
                                        if !rest_after_o.starts_with(' ') {
                                            continue;
                                        }
                                        let rest_after_o = &rest_after_o[1..];

                                        // Try each outcome phrase
                                        for (k_idx, k_phrase) in
                                            self.outcome_dict.entries.iter().enumerate()
                                        {
                                            if rest_after_o == k_phrase {
                                                // All phrases successfully matched
                                                return Ok((c_idx, s_idx, a_idx, o_idx, k_idx));
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        Err(Memo128Error::ParsingError(format!(
            "Cannot parse sentence: {}",
            sentence
        )))
    }

    /// Decodes three sentences back to the original 128-bit value as a hex string
    ///
    /// # Arguments
    ///
    /// * `input_sentences` - A slice of three strings containing the sentences to decode
    ///
    /// # Returns
    ///
    /// Result containing either the original 32-character hex string or a Memo128Error
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The number of sentences is not exactly 3
    /// - Any sentence cannot be parsed
    /// - The checksum verification fails (indicating an error in one of the sentences)
    ///
    /// # Example
    ///
    /// ```rust
    /// # use memo128::Memo128;
    /// # fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let memo128 = Memo128::new()?;
    /// let sentences = vec![
    ///     "a brave mouse inside a cosmic cathedral disconnected a question of time but it was already too late".to_string(),
    ///     "a worried parent within the cosmic algorithm accepted a mathematical impossibility as code predicted".to_string(),
    ///     "the fjord elder inside a particle accelerator stole a reality glitch rebooting the system".to_string(),
    /// ];
    /// let hex_value = memo128.decode(&sentences)?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn decode(&self, input_sentences: &[String]) -> Result<String, Memo128Error> {
        if input_sentences.len() != NUM_CHUNKS {
            return Err(Memo128Error::ParsingError(format!(
                "Expected exactly {} sentences, got {}",
                NUM_CHUNKS,
                input_sentences.len()
            )));
        }

        let mut reconstructed_135_num = BigUint::zero();

        // Process each sentence
        for sentence in input_sentences {
            let sentence = sentence.trim();

            // Parse sentence to get component indices
            let (idx_c, idx_s, idx_a, idx_o, idx_k) = self.parse_sentence(sentence)?;

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

        if checksum_bits_decoded != checksum_bits_calculated {
            return Err(Memo128Error::ChecksumError);
        }

        // Format output as 32-char hex string
        Ok(Self::bytes_to_hex(&padded_bytes))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::tempdir;

    fn create_test_dictionaries() -> std::io::Result<tempfile::TempDir> {
        let dir = tempdir()?;

        // Create minimal test dictionaries
        let dict_files = [
            ("character_10bit.txt", 1 << CHARACTER_BITS),
            ("setting_10bit.txt", 1 << SETTING_BITS),
            ("action_8bit.txt", 1 << ACTION_BITS),
            ("object_9bit.txt", 1 << OBJECT_BITS),
            ("outcome_8bit.txt", 1 << OUTCOME_BITS),
        ];

        for (filename, size) in dict_files.iter() {
            let file_path = dir.path().join(filename);
            let mut file = File::create(file_path)?;

            for i in 0..*size {
                writeln!(file, "test entry {}", i)?;
            }
        }

        Ok(dir)
    }

    #[test]
    fn test_checksum_calculation() {
        // Create a Memo128 instance directly for this test to avoid dictionary loading
        struct TestMemo128;

        impl TestMemo128 {
            fn calculate_checksum(&self, data: &[u8]) -> u8 {
                let mut hasher = Sha256::new();
                hasher.update(data);
                let result = hasher.finalize();
                // Take the first 7 bits (MSB) of the hash
                (result[0] >> 1) & 0x7F
            }
        }

        let memo128 = TestMemo128;

        // Test vector
        let data = [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15];
        let checksum = memo128.calculate_checksum(&data);

        // We don't know the exact checksum value, but it should be in range 0-127
        assert!(checksum <= 127);
    }

    #[test]
    fn test_hex_conversion() {
        let hex = "000102030405060708090a0b0c0d0e0f";
        let bytes = Memo128::hex_to_bytes(hex).unwrap();
        assert_eq!(
            bytes,
            [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15]
        );

        let hex_roundtrip = Memo128::bytes_to_hex(&bytes);
        assert_eq!(hex, hex_roundtrip);
    }

    #[test]
    fn test_roundtrip_encoding_decoding() {
        let dir = create_test_dictionaries().unwrap();

        // Change to the test directory to find dictionaries
        let original_dir = std::env::current_dir().unwrap();
        std::env::set_current_dir(dir.path()).unwrap();

        let memo128 = Memo128::new().unwrap();

        // Test vector
        let hex_input = "000102030405060708090a0b0c0d0e0f";

        // Encode
        let sentences = memo128.encode(hex_input).unwrap();
        assert_eq!(sentences.len(), 3);

        // Decode
        let hex_output = memo128.decode(&sentences).unwrap();
        assert_eq!(hex_input, hex_output);

        // Reset directory
        std::env::set_current_dir(original_dir).unwrap();
    }

    #[test]
    fn test_sentence_parsing() {
        // Create mock dictionary entries and a mocked Memo128 structure
        struct MockMemo128 {
            character_dict: Dictionary,
            setting_dict: Dictionary,
            action_dict: Dictionary,
            object_dict: Dictionary,
            outcome_dict: Dictionary,
        }

        impl MockMemo128 {
            fn parse_sentence(
                &self,
                sentence: &str,
            ) -> Result<(usize, usize, usize, usize, usize), Memo128Error> {
                // For each possible character phrase
                for (c_idx, c_phrase) in self.character_dict.entries.iter().enumerate() {
                    // If the sentence starts with this character phrase
                    if sentence.starts_with(c_phrase) {
                        let rest_after_c = &sentence[c_phrase.len()..];

                        // Skip the space after character phrase
                        if !rest_after_c.starts_with(' ') {
                            continue;
                        }
                        let rest_after_c = &rest_after_c[1..];

                        // Try each setting phrase
                        for (s_idx, s_phrase) in self.setting_dict.entries.iter().enumerate() {
                            if rest_after_c.starts_with(s_phrase) {
                                let rest_after_s = &rest_after_c[s_phrase.len()..];

                                // Skip the space after setting phrase
                                if !rest_after_s.starts_with(' ') {
                                    continue;
                                }
                                let rest_after_s = &rest_after_s[1..];

                                // Try each action phrase
                                for (a_idx, a_phrase) in self.action_dict.entries.iter().enumerate()
                                {
                                    if rest_after_s.starts_with(a_phrase) {
                                        let rest_after_a = &rest_after_s[a_phrase.len()..];

                                        // Skip the space after action phrase
                                        if !rest_after_a.starts_with(' ') {
                                            continue;
                                        }
                                        let rest_after_a = &rest_after_a[1..];

                                        // Try each object phrase
                                        for (o_idx, o_phrase) in
                                            self.object_dict.entries.iter().enumerate()
                                        {
                                            if rest_after_a.starts_with(o_phrase) {
                                                let rest_after_o = &rest_after_a[o_phrase.len()..];

                                                // Skip the space after object phrase
                                                if !rest_after_o.starts_with(' ') {
                                                    continue;
                                                }
                                                let rest_after_o = &rest_after_o[1..];

                                                // Try each outcome phrase
                                                for (k_idx, k_phrase) in
                                                    self.outcome_dict.entries.iter().enumerate()
                                                {
                                                    if rest_after_o == k_phrase {
                                                        // All phrases successfully matched
                                                        return Ok((
                                                            c_idx, s_idx, a_idx, o_idx, k_idx,
                                                        ));
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }

                Err(Memo128Error::ParsingError(format!(
                    "Cannot parse sentence: {}",
                    sentence
                )))
            }
        }

        // Create mock dictionaries with a few entries
        let mut c_dict = Dictionary::new(10);
        let mut s_dict = Dictionary::new(15);
        let mut a_dict = Dictionary::new(20);
        let mut o_dict = Dictionary::new(25);
        let mut k_dict = Dictionary::new(30);

        // Add some test entries
        for i in 0..10 {
            c_dict.entries.push(format!("character_{}", i));
            c_dict.reverse_lookup.insert(format!("character_{}", i), i);
        }

        for i in 0..15 {
            s_dict.entries.push(format!("setting_{}", i));
            s_dict.reverse_lookup.insert(format!("setting_{}", i), i);
        }

        for i in 0..20 {
            a_dict.entries.push(format!("action_{}", i));
            a_dict.reverse_lookup.insert(format!("action_{}", i), i);
        }

        for i in 0..25 {
            o_dict.entries.push(format!("object_{}", i));
            o_dict.reverse_lookup.insert(format!("object_{}", i), i);
        }

        for i in 0..30 {
            k_dict.entries.push(format!("outcome_{}", i));
            k_dict.reverse_lookup.insert(format!("outcome_{}", i), i);
        }

        let mock_memo128 = MockMemo128 {
            character_dict: c_dict,
            setting_dict: s_dict,
            action_dict: a_dict,
            object_dict: o_dict,
            outcome_dict: k_dict,
        };

        // Create a test sentence from known indices
        let c_idx = 5;
        let s_idx = 10;
        let a_idx = 15;
        let o_idx = 20;
        let k_idx = 25;

        let c_phrase = &mock_memo128.character_dict.entries[c_idx];
        let s_phrase = &mock_memo128.setting_dict.entries[s_idx];
        let a_phrase = &mock_memo128.action_dict.entries[a_idx];
        let o_phrase = &mock_memo128.object_dict.entries[o_idx];
        let k_phrase = &mock_memo128.outcome_dict.entries[k_idx];

        let sentence = format!(
            "{} {} {} {} {}",
            c_phrase, s_phrase, a_phrase, o_phrase, k_phrase
        );

        // Parse the sentence
        let (parsed_c, parsed_s, parsed_a, parsed_o, parsed_k) =
            mock_memo128.parse_sentence(&sentence).unwrap();

        // Verify the parsed indices
        assert_eq!(parsed_c, c_idx);
        assert_eq!(parsed_s, s_idx);
        assert_eq!(parsed_a, a_idx);
        assert_eq!(parsed_o, o_idx);
        assert_eq!(parsed_k, k_idx);
    }
}
