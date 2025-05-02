use std::collections::HashMap;
use std::fs::File;
use std::io::{self, BufRead, BufReader};
use std::num::ParseIntError;
use std::path::Path;
use sha2::{Sha256, Digest};
use num_bigint::BigUint;
use num_traits::cast::ToPrimitive;

const CHARACTER_DICT_PATH: &str = "character_10bit.txt";
const SETTING_DICT_PATH: &str = "setting_10bit.txt";
const ACTION_DICT_PATH: &str = "action_8bit.txt";
const OBJECT_DICT_PATH: &str = "object_9bit.txt";
const OUTCOME_DICT_PATH: &str = "outcome_8bit.txt";

// Error types for the encoder/decoder
#[derive(Debug)]
#[allow(dead_code)]  // Allow unused fields since we need them for error context
enum Error {
    IoError(io::Error),
    InvalidHexInput(ParseIntError),
    InvalidDictionary(String),
    InvalidSentenceFormat(String),
    WordNotFound(String),
    ChecksumError,
    Other(String),
}

impl From<io::Error> for Error {
    fn from(err: io::Error) -> Self {
        Error::IoError(err)
    }
}

impl From<ParseIntError> for Error {
    fn from(err: ParseIntError) -> Self {
        Error::InvalidHexInput(err)
    }
}

// Dictionary structure to hold word lists and provide forward/reverse lookups
struct Dictionary {
    name: String,
    words: Vec<String>,
    reverse_lookup: HashMap<String, usize>,
    bit_size: u8,
}

impl Dictionary {
    fn new(name: &str, bit_size: u8) -> Self {
        Dictionary {
            name: name.to_string(),
            words: Vec::new(),
            reverse_lookup: HashMap::new(),
            bit_size,
        }
    }

    fn expected_size(&self) -> usize {
        1 << self.bit_size
    }

    fn load<P: AsRef<Path>>(&mut self, path: P) -> Result<(), Error> {
        let file = File::open(&path).map_err(|e| {
            Error::IoError(io::Error::new(
                io::ErrorKind::NotFound,
                format!("Failed to open {}: {}", self.name, e),
            ))
        })?;

        let reader = BufReader::new(file);
        self.words.clear();
        self.reverse_lookup.clear();

        for (idx, line) in reader.lines().enumerate() {
            let word = line?.trim().to_string();
            if word.is_empty() {
                return Err(Error::InvalidDictionary(format!(
                    "Empty word at line {} in {}",
                    idx + 1, self.name
                )));
            }

            if self.reverse_lookup.contains_key(&word) {
                return Err(Error::InvalidDictionary(format!(
                    "Duplicate word '{}' in {}",
                    word, self.name
                )));
            }

            self.reverse_lookup.insert(word.clone(), idx);
            self.words.push(word);
        }

        // Validate dictionary size
        let expected_size = self.expected_size();
        if self.words.len() != expected_size {
            return Err(Error::InvalidDictionary(format!(
                "Dictionary {} has {} entries, expected {}",
                self.name,
                self.words.len(),
                expected_size
            )));
        }

        Ok(())
    }

    fn get_word(&self, index: usize) -> Result<&String, Error> {
        self.words.get(index).ok_or_else(|| {
            Error::InvalidDictionary(format!(
                "Index {} out of bounds for {} dictionary (size {})",
                index,
                self.name,
                self.words.len()
            ))
        })
    }

    fn get_index(&self, word: &str) -> Result<usize, Error> {
        self.reverse_lookup.get(word).copied().ok_or_else(|| {
            Error::WordNotFound(format!("Word '{}' not found in {} dictionary", word, self.name))
        })
    }
}

// Main encoder/decoder structure
struct SentenceEncoder {
    character_dict: Dictionary,
    setting_dict: Dictionary,
    action_dict: Dictionary,
    object_dict: Dictionary,
    outcome_dict: Dictionary,
}

impl SentenceEncoder {
    fn new() -> Self {
        SentenceEncoder {
            character_dict: Dictionary::new("character", 10),
            setting_dict: Dictionary::new("setting", 10),
            action_dict: Dictionary::new("action", 8),
            object_dict: Dictionary::new("object", 9),
            outcome_dict: Dictionary::new("outcome", 8),
        }
    }

    fn load_dictionaries(&mut self) -> Result<(), Error> {
        self.character_dict.load(CHARACTER_DICT_PATH)?;
        self.setting_dict.load(SETTING_DICT_PATH)?;
        self.action_dict.load(ACTION_DICT_PATH)?;
        self.object_dict.load(OBJECT_DICT_PATH)?;
        self.outcome_dict.load(OUTCOME_DICT_PATH)?;
        Ok(())
    }

    // Calculate 7-bit checksum from 128-bit data
    fn calculate_checksum(&self, data_bytes: &[u8]) -> u8 {
        let mut hasher = Sha256::new();
        hasher.update(data_bytes);
        let result = hasher.finalize();
        // Take first byte and keep only 7 most significant bits
        (result[0] >> 1) & 0x7F
    }

    // Encode a 128-bit integer (represented as a hex string) to 3 sentences
    fn encode(&self, hex_input: &str) -> Result<Vec<String>, Error> {
        // 1. Validate input format
        if hex_input.len() != 32 || !hex_input.chars().all(|c| c.is_ascii_hexdigit()) {
            return Err(Error::Other(format!(
                "Invalid input: expected 32-char hex string, got '{}'",
                hex_input
            )));
        }

        // 2. Convert hex to bytes
        let mut data_bytes = [0u8; 16];
        for i in 0..16 {
            let start = i * 2;
            let byte_str = &hex_input[start..start + 2];
            data_bytes[i] = u8::from_str_radix(byte_str, 16)?;
        }

        // 3. Calculate checksum
        let checksum_bits = self.calculate_checksum(&data_bytes);

        // 4. Convert to BigUint (128-bit integer)
        let data_num = BigUint::from_bytes_be(&data_bytes);

        // 5. Combine data and checksum into 135-bit number N
        let combined_num = (data_num << 7) | BigUint::from(checksum_bits);

        // 6. Initialize output
        let mut output_sentences = Vec::with_capacity(3);

        // 7. Process 3 chunks
        for chunk_idx in 0..3 {
            // Extract chunk - each chunk is 45 bits (135 bits total split into 3)
            let shift_bits = (2 - chunk_idx) * 45;
            let mask = (BigUint::from(1u64) << 45) - 1u64;
            let chunk_value: BigUint = (&combined_num >> shift_bits) & &mask;

            // Convert to u64 for easier bit manipulation
            let chunk_u64 = chunk_value.to_u64().unwrap_or(0);
            
            // Extract indices - the chunk is 45 bits broken down as 10+10+8+9+8 = 45 bits
            let idx_c = (chunk_u64 >> 35) & 0x3FF;  // 10 bits for character (bits 35-44)
            let idx_s = (chunk_u64 >> 25) & 0x3FF;  // 10 bits for setting (bits 25-34)
            let idx_a = (chunk_u64 >> 17) & 0xFF;   // 8 bits for action (bits 17-24)
            let idx_o = (chunk_u64 >> 8) & 0x1FF;   // 9 bits for object (bits 8-16)
            let idx_k = chunk_u64 & 0xFF;           // 8 bits for outcome (bits 0-7)

            // Lookup words from dictionaries
            let word_c = self.character_dict.get_word(idx_c as usize)?;
            let word_s = self.setting_dict.get_word(idx_s as usize)?;
            let word_a = self.action_dict.get_word(idx_a as usize)?;
            let word_o = self.object_dict.get_word(idx_o as usize)?;
            let word_k = self.outcome_dict.get_word(idx_k as usize)?;

            // Assemble sentence
            let sentence = format!("{} {} {} {} {}", word_c, word_s, word_a, word_o, word_k);
            output_sentences.push(sentence);
        }

        Ok(output_sentences)
    }

    // Decode 3 sentences back to a 128-bit integer (represented as a hex string)
    fn decode(&self, input_sentences: &[String]) -> Result<String, Error> {
        // 1. Validate input format
        if input_sentences.len() != 3 {
            return Err(Error::Other(format!(
                "Invalid input: expected 3 sentences, got {}",
                input_sentences.len()
            )));
        }

        // 2. Initialize 135-bit number
        let mut reconstructed_135_num = BigUint::from(0u32);

        // 3. Process 3 sentences in order
        for (i, sentence) in input_sentences.iter().enumerate() {
            let parts: Vec<&str> = sentence.split_whitespace().collect();
            if parts.len() != 5 {
                return Err(Error::InvalidSentenceFormat(format!(
                    "Expected 5 words in sentence, found {}: '{}'",
                    parts.len(),
                    sentence
                )));
            }

            // Reverse lookup indices
            let idx_c = self.character_dict.get_index(parts[0])?;
            let idx_s = self.setting_dict.get_index(parts[1])?;
            let idx_a = self.action_dict.get_index(parts[2])?;
            let idx_o = self.object_dict.get_index(parts[3])?;
            let idx_k = self.outcome_dict.get_index(parts[4])?;

            // Validate indices
            if idx_c >= (1 << 10) || idx_s >= (1 << 10) || idx_a >= (1 << 8) ||
                idx_o >= (1 << 9) || idx_k >= (1 << 8) {
                return Err(Error::Other("Index out of range for component".to_string()));
            }

            // Reconstruct chunk value (45 bits)
            let chunk_value = BigUint::from(idx_c) << 35 |
                BigUint::from(idx_s) << 25 |
                BigUint::from(idx_a) << 17 |
                BigUint::from(idx_o) << 8 |
                BigUint::from(idx_k);

            // Based on the sentence position, place the chunk in the appropriate position
            // First sentence (i=0) is the leftmost 45 bits of the 135-bit number
            let shift_amount = (2 - i) * 45;
            reconstructed_135_num |= chunk_value << shift_amount;
        }

        // 4. Separate data and checksum
        let checksum_bits_decoded = &reconstructed_135_num & BigUint::from(0x7Fu32);
        let data_num_decoded: BigUint = &reconstructed_135_num >> 7;

        // 5. Convert data_num_decoded to bytes
        let data_bytes_decoded = data_num_decoded.to_bytes_be();

        // Ensure we have exactly 16 bytes
        let mut padded_bytes = vec![0u8; 16];
        let offset = 16 - data_bytes_decoded.len();
        padded_bytes[offset..(data_bytes_decoded.len() + offset)].copy_from_slice(&data_bytes_decoded[..]);

        // 6. Verify checksum
        let calculated_checksum = self.calculate_checksum(&padded_bytes);
        let decoded_checksum = checksum_bits_decoded.to_u8().unwrap();

        if decoded_checksum != calculated_checksum {
            return Err(Error::ChecksumError);
        }

        // 7. Format output as 32-character hex string
        let mut hex_output = String::new();
        for byte in padded_bytes {
            hex_output.push_str(&format!("{:02x}", byte));
        }

        Ok(hex_output)
    }
}

fn main() -> Result<(), Error> {
    let args: Vec<String> = std::env::args().collect();

    if args.len() < 2 {
        println!("Usage:");
        println!("  encode <32-char-hex>              - Encode 128-bit hex to 3 sentences");
        println!("  decode \"<sentence1>\" \"<sentence2>\" \"<sentence3>\"  - Decode 3 sentences to 128-bit hex");
        return Ok(());
    }

    let mut encoder = SentenceEncoder::new();
    encoder.load_dictionaries()?;

    match args[1].as_str() {
        "encode" => {
            if args.len() != 3 {
                return Err(Error::Other("Encode requires a 32-character hex string".to_string()));
            }
            let hex_input = args[2].to_lowercase();
            let sentences = encoder.encode(&hex_input)?;
            println!("Encoded sentences:");
            for (i, sentence) in sentences.iter().enumerate() {
                println!("Sentence {}: {}", i + 1, sentence);
            }
        },
        "decode" => {
            if args.len() != 5 {
                return Err(Error::Other("Decode requires exactly 3 sentences".to_string()));
            }
            let sentences = vec![args[2].clone(), args[3].clone(), args[4].clone()];
            let hex_output = encoder.decode(&sentences)?;
            println!("Decoded hex: {}", hex_output);
        },
        _ => {
            return Err(Error::Other(format!("Unknown command: {}", args[1])));
        }
    }

    Ok(())
}

// Tests
#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::tempdir;

    // Extension method for Dictionary to load from a full path
    impl Dictionary {
        fn load_with_full_path<P: AsRef<Path>>(&mut self, full_path: P) -> Result<(), Error> {
            let file = File::open(&full_path).map_err(|e| {
                Error::IoError(io::Error::new(
                    io::ErrorKind::NotFound,
                    format!("Failed to open {}: {}", self.name, e),
                ))
            })?;

            let reader = BufReader::new(file);
            self.words.clear();
            self.reverse_lookup.clear();

            for (idx, line) in reader.lines().enumerate() {
                let word = line?.trim().to_string();
                if word.is_empty() {
                    return Err(Error::InvalidDictionary(format!(
                        "Empty word at line {} in {}",
                        idx + 1, self.name
                    )));
                }

                if self.reverse_lookup.contains_key(&word) {
                    return Err(Error::InvalidDictionary(format!(
                        "Duplicate word '{}' in {}",
                        word, self.name
                    )));
                }

                self.reverse_lookup.insert(word.clone(), idx);
                self.words.push(word);
            }

            // Validate dictionary size
            let expected_size = self.expected_size();
            if self.words.len() != expected_size {
                return Err(Error::InvalidDictionary(format!(
                    "Dictionary {} has {} entries, expected {}",
                    self.name,
                    self.words.len(),
                    expected_size
                )));
            }

            Ok(())
        }
    }

    // Helper function to create test dictionaries and set up an encoder
    fn create_test_encoder() -> (tempfile::TempDir, SentenceEncoder) {
        let dir = tempdir().unwrap();

        // Create character dictionary (10-bit = 1024 entries)
        let char_path = dir.path().join(CHARACTER_DICT_PATH);
        let mut char_file = File::create(&char_path).unwrap();
        for i in 0..1024 {
            writeln!(char_file, "character_{}", i).unwrap();
        }

        // Create setting dictionary (10-bit = 1024 entries)
        let setting_path = dir.path().join(SETTING_DICT_PATH);
        let mut setting_file = File::create(&setting_path).unwrap();
        for i in 0..1024 {
            writeln!(setting_file, "setting_{}", i).unwrap();
        }

        // Create action dictionary (8-bit = 256 entries)
        let action_path = dir.path().join(ACTION_DICT_PATH);
        let mut action_file = File::create(&action_path).unwrap();
        for i in 0..256 {
            writeln!(action_file, "action_{}", i).unwrap();
        }

        // Create object dictionary (9-bit = 512 entries)
        let object_path = dir.path().join(OBJECT_DICT_PATH);
        let mut object_file = File::create(&object_path).unwrap();
        for i in 0..512 {
            writeln!(object_file, "object_{}", i).unwrap();
        }

        // Create outcome dictionary (8-bit = 256 entries)
        let outcome_path = dir.path().join(OUTCOME_DICT_PATH);
        let mut outcome_file = File::create(&outcome_path).unwrap();
        for i in 0..256 {
            writeln!(outcome_file, "outcome_{}", i).unwrap();
        }

        // Create and set up the encoder
        let mut encoder = SentenceEncoder::new();
        
        // Load dictionaries with full paths
        encoder.character_dict.load_with_full_path(char_path).unwrap();
        encoder.setting_dict.load_with_full_path(setting_path).unwrap();
        encoder.action_dict.load_with_full_path(action_path).unwrap();
        encoder.object_dict.load_with_full_path(object_path).unwrap();
        encoder.outcome_dict.load_with_full_path(outcome_path).unwrap();

        (dir, encoder)
    }

    #[test]
    fn test_encode_decode_roundtrip() {
        // Create test dictionaries and get encoder
        let (_dir, encoder) = create_test_encoder();

        // Test with a known 128-bit hex value
        let test_hex = "0123456789abcdef0123456789abcdef";

        // Encode
        let sentences = encoder.encode(test_hex).unwrap();
        assert_eq!(sentences.len(), 3);

        // Decode
        let decoded_hex = encoder.decode(&sentences).unwrap();
        
        // Verify that the decoded hex matches the original
        assert_eq!(decoded_hex, test_hex);
        
        // Test another value to be thorough
        let test_hex2 = "ffffffffffffffffffffffffffffffff";
        let sentences2 = encoder.encode(test_hex2).unwrap();
        let decoded_hex2 = encoder.decode(&sentences2).unwrap();
        assert_eq!(decoded_hex2, test_hex2);
    }

    #[test]
    fn test_checksum_validation() {
        // Create test dictionaries and get encoder
        let (_dir, encoder) = create_test_encoder();

        // Generate a valid encoding
        let test_hex = "ffffffffffffffffffffffffffffffff";
        let sentences = encoder.encode(test_hex).unwrap();

        // Modify a sentence to cause checksum failure
        let mut bad_sentences = sentences.clone();
        
        // Get the parts of the first sentence
        let parts: Vec<&str> = bad_sentences[0].split_whitespace().collect();
        
        // Get a different character word than the original
        // The word format in our test dictionaries is "character_X" where X is an index
        // So we'll just use a different index
        let original_index = parts[0].strip_prefix("character_").unwrap_or("0");
        let original_index = original_index.parse::<usize>().unwrap_or(0);
        let new_index = (original_index + 1) % 1024; // Ensure it's different
        
        // Change the character word to something that exists but is different
        bad_sentences[0] = format!("character_{} {} {} {} {}", new_index, parts[1], parts[2], parts[3], parts[4]);

        // Decoding should fail with checksum error
        let result = encoder.decode(&bad_sentences);
        assert!(matches!(result, Err(Error::ChecksumError)));
    }
}
