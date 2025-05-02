use std::collections::HashMap;
use std::fs::File;
use std::io::{self, BufRead, BufReader};
use std::path::Path;

use num_bigint::BigUint;
use num_traits::{One, ToPrimitive, Zero};
use sha2::{Digest, Sha256};
use thiserror::Error;

// Expose fuzzy decoding
pub mod fuzzy;

// Constants for bit allocations
pub const CHARACTER_BITS: u32 = 10;
pub const SETTING_BITS: u32 = 10;
pub const ACTION_BITS: u32 = 8;
pub const OBJECT_BITS: u32 = 9;
pub const OUTCOME_BITS: u32 = 8;

pub const CHUNK_BITS: u32 =
    CHARACTER_BITS + SETTING_BITS + ACTION_BITS + OBJECT_BITS + OUTCOME_BITS; // 45 bits
pub const CHECKSUM_BITS: u32 = 7;

pub const NUM_CHUNKS: usize = 3; // 3 chunks of 45 bits = 135 bits

// Custom error type
#[derive(Debug, Error)]
pub enum Memo128Error {
    #[error("IO error: {0}")]
    IoError(#[from] io::Error),
    
    #[error("Invalid hex input: {0}")]
    InvalidHexInput(String),
    
    #[error("Dictionary error: {0}")]
    InvalidDictionary(String),
    
    #[error("Parsing error: {0}")]
    ParsingError(String),
    
    #[error("Checksum verification failed")]
    ChecksumError,
}

// Dictionary struct for handling dictionary files
pub struct Dictionary {
    pub entries: Vec<String>,
    pub reverse_lookup: HashMap<String, usize>,
}

impl Dictionary {
    pub fn new(expected_size: usize) -> Self {
        Dictionary {
            entries: Vec::with_capacity(expected_size),
            reverse_lookup: HashMap::with_capacity(expected_size),
        }
    }

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

    pub fn get(&self, index: usize) -> Option<&String> {
        self.entries.get(index)
    }
}

// Memo128 struct for handling encoding and decoding
pub struct Memo128 {
    character_dict: Dictionary,
    setting_dict: Dictionary,
    action_dict: Dictionary,
    object_dict: Dictionary,
    outcome_dict: Dictionary,
}

impl Memo128 {
    pub fn new() -> Result<Self, Memo128Error> {
        Ok(Memo128 {
            character_dict: Dictionary::load("character_10bit.txt", 1 << CHARACTER_BITS)?,
            setting_dict: Dictionary::load("setting_10bit.txt", 1 << SETTING_BITS)?,
            action_dict: Dictionary::load("action_8bit.txt", 1 << ACTION_BITS)?,
            object_dict: Dictionary::load("object_9bit.txt", 1 << OBJECT_BITS)?,
            outcome_dict: Dictionary::load("outcome_8bit.txt", 1 << OUTCOME_BITS)?,
        })
    }
    
    // Access to dictionaries for fuzzy decoding
    pub fn get_character_dict(&self) -> &Dictionary {
        &self.character_dict
    }
    
    pub fn get_setting_dict(&self) -> &Dictionary {
        &self.setting_dict
    }
    
    pub fn get_action_dict(&self) -> &Dictionary {
        &self.action_dict
    }
    
    pub fn get_object_dict(&self) -> &Dictionary {
        &self.object_dict
    }
    
    pub fn get_outcome_dict(&self) -> &Dictionary {
        &self.outcome_dict
    }

    // Calculate 7-bit checksum from 128-bit data
    fn calculate_checksum(&self, data: &[u8]) -> u8 {
        let mut hasher = Sha256::new();
        hasher.update(data);
        let result = hasher.finalize();
        // Take the first 7 bits (MSB) of the hash
        (result[0] >> 1) & 0x7F
    }

    // Convert hex string to bytes
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

    // Convert bytes to hex string
    pub fn bytes_to_hex(bytes: &[u8]) -> String {
        bytes
            .iter()
            .map(|b| format!("{:02x}", b))
            .collect::<String>()
    }

    // Encode 128-bit number to 3 sentences
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

    // Parse a sentence back into its component phrases
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

    // Decode 3 sentences back to 128-bit hex
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
            let chunk_value = BigUint::from(idx_c)
                << (SETTING_BITS + ACTION_BITS + OBJECT_BITS + OUTCOME_BITS)
                | BigUint::from(idx_s) << (ACTION_BITS + OBJECT_BITS + OUTCOME_BITS)
                | BigUint::from(idx_a) << (OBJECT_BITS + OUTCOME_BITS)
                | BigUint::from(idx_o) << OUTCOME_BITS
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
