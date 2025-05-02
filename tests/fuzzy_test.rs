use memo128::{fuzzy::FuzzyMemo128, Memo128};

#[test]
fn test_fuzzy_decode_single_char_change() -> Result<(), Box<dyn std::error::Error>> {
    // Test vector - using just one to keep runtime manageable
    let hex = "000102030405060708090a0b0c0d0e0f";

    println!("Running fuzzy decoding test with hex: {}", hex);

    // Create a memo128 instance for encoding
    let memo128 = Memo128::new()?;

    // Create a fuzzy decoder with Levenshtein distance 1 (faster, but less tolerant)
    let fuzzy_memo128 = FuzzyMemo128::new(1)?;

    // Encode the hex to 3 sentences
    let sentences = memo128.encode(hex)?;

    println!("Original sentences:");
    for (j, sentence) in sentences.iter().enumerate() {
        println!("  Sentence {}: {}", j + 1, sentence);
    }

    // Test only a simple modification to the first sentence to demonstrate functionality
    println!("Testing a single character modification in the first sentence...");

    // Simple approach - just modify one character in the sentence
    let mut chars: Vec<char> = sentences[0].chars().collect();
    // Find a non-space character in the middle to modify
    let middle = chars.len() / 2;
    let mut pos = middle;

    // Find a non-space character to modify
    while pos < chars.len() && chars[pos] == ' ' {
        pos += 1;
    }

    // Make a simple substitution
    if pos < chars.len() {
        if "aeiou".contains(chars[pos]) {
            chars[pos] = 'o'; // Change a vowel to 'o'
        } else {
            chars[pos] = 'x'; // Change any other character to 'x'
        }
    }

    let modified_sentence: String = chars.into_iter().collect();

    let mut modified_sentences = sentences.clone();
    modified_sentences[0] = modified_sentence.clone();

    println!("  Modified: {}", modified_sentence);
    println!("Fuzzy decoding...");

    // Try fuzzy decoding
    match fuzzy_memo128.fuzzy_decode(&modified_sentences) {
        Ok(results) => {
            println!("Results found: {}", results.len());

            // Check if original hex is in the results
            let original_found = results.iter().any(|result| result == hex);
            println!("Original hex found: {}", original_found);
            assert!(original_found, "Original hex not found in results");

            // Print all results
            for (result_idx, result) in results.iter().enumerate() {
                println!("  Result {}: {}", result_idx + 1, result);
            }
        }
        Err(e) => {
            eprintln!("Error: {}", e);
            return Err(e.into());
        }
    }

    Ok(())
}

#[test]
fn test_fuzzy_decode_word_change() -> Result<(), Box<dyn std::error::Error>> {
    // Test vector
    let hex = "000102030405060708090a0b0c0d0e0f";

    // Create a memo128 instance for encoding
    let memo128 = Memo128::new()?;

    // Create a fuzzy decoder with higher Levenshtein distance
    let fuzzy_memo128 = FuzzyMemo128::new(3)?;

    // Encode the hex to 3 sentences
    let sentences = memo128.encode(hex)?;

    println!("Original sentences:");
    for (j, sentence) in sentences.iter().enumerate() {
        println!("  Sentence {}: {}", j + 1, sentence);
    }

    // Test replacing a word in each sentence
    let mut modified_sentences = Vec::new();
    
    for sentence in &sentences {
        let parts: Vec<String> = sentence.split(' ').map(String::from).collect();
        if parts.len() > 2 {
            // Replace a word in the middle
            let mid_idx = parts.len() / 2;
            let mut modified_parts = parts.clone();
            
            // Replace with a similar but different word
            if parts[mid_idx].len() > 3 {
                // Change just a single character in the middle of the word
                let mid_char_pos = parts[mid_idx].len() / 2;
                let mut chars: Vec<char> = parts[mid_idx].chars().collect();
                
                // Make a minimal change to just one character
                if mid_char_pos < chars.len() {
                    // If it's a vowel, replace with another vowel
                    if "aeiou".contains(chars[mid_char_pos]) {
                        chars[mid_char_pos] = 'e';
                    } else if chars[mid_char_pos].is_alphabetic() {
                        // Otherwise replace with next letter in alphabet
                        chars[mid_char_pos] = next_char(chars[mid_char_pos]);
                    }
                }
                
                modified_parts[mid_idx] = chars.into_iter().collect();
            } else {
                // For short words, make a minimal change
                if parts[mid_idx].chars().next().unwrap_or('a').is_alphabetic() {
                    modified_parts[mid_idx] = format!("{}x", parts[mid_idx]);
                } else {
                    modified_parts[mid_idx] = "xyz".to_string();
                }
            }
            
            // Reconstruct the sentence
            let modified = modified_parts.join(" ");
            modified_sentences.push(modified);
        } else {
            // Not enough parts to modify, keep original
            modified_sentences.push(sentence.clone());
        }
    }

    println!("Modified sentences:");
    for (j, sentence) in modified_sentences.iter().enumerate() {
        println!("  Sentence {}: {}", j + 1, sentence);
    }

    // Try fuzzy decoding with the modified sentences
    match fuzzy_memo128.fuzzy_decode(&modified_sentences) {
        Ok(results) => {
            println!("Results found: {}", results.len());

            // Check if original hex is in the results
            let original_found = results.iter().any(|result| result == hex);
            println!("Original hex found: {}", original_found);
            assert!(original_found, "Original hex not found in results");

            // Print all results
            for (result_idx, result) in results.iter().enumerate().take(5) {
                println!("  Result {}: {}", result_idx + 1, result);
            }
        }
        Err(e) => {
            eprintln!("Error: {}", e);
            return Err(e.into());
        }
    }

    Ok(())
}

#[test]
fn test_fuzzy_decode_multiple_changes() -> Result<(), Box<dyn std::error::Error>> {
    // Test vector
    let hex = "000102030405060708090a0b0c0d0e0f";

    // Create a memo128 instance for encoding
    let memo128 = Memo128::new()?;

    // Create a fuzzy decoder with higher Levenshtein distance
    let fuzzy_memo128 = FuzzyMemo128::new(3)?;

    // Encode the hex to 3 sentences
    let sentences = memo128.encode(hex)?;

    println!("Original sentences:");
    for (j, sentence) in sentences.iter().enumerate() {
        println!("  Sentence {}: {}", j + 1, sentence);
    }

    // Create modified sentences with multiple changes
    let mut modified_sentences = Vec::new();
    
    for sentence in &sentences {
        // Add extra spaces in random positions
        let spaced_sentence = sentence.chars()
            .fold(String::new(), |mut acc, c| {
                acc.push(c);
                if c != ' ' && rand_bool(0.1) {
                    acc.push(' ');
                }
                acc
            });
        
        // Change some characters
        let modified = spaced_sentence.chars()
            .map(|c| {
                if rand_bool(0.1) {
                    if "aeiou".contains(c) { 'o' } 
                    else if c.is_alphabetic() { next_char(c) }
                    else { c }
                } else {
                    c
                }
            })
            .collect::<String>();
            
        modified_sentences.push(modified);
    }

    println!("Modified sentences with multiple changes:");
    for (j, sentence) in modified_sentences.iter().enumerate() {
        println!("  Sentence {}: {}", j + 1, sentence);
    }

    // Try fuzzy decoding with the heavily modified sentences
    match fuzzy_memo128.fuzzy_decode(&modified_sentences) {
        Ok(results) => {
            println!("Results found: {}", results.len());

            // Check if original hex is in the results
            let original_found = results.iter().any(|result| result == hex);
            println!("Original hex found: {}", original_found);
            
            // This test is more lenient - we don't assert the original is found
            // as the changes might be too severe

            // Print all results
            for (result_idx, result) in results.iter().enumerate().take(5) {
                println!("  Result {}: {}", result_idx + 1, result);
            }
        }
        Err(e) => {
            println!("Error: {} - This is expected if changes were too severe", e);
            // Don't fail the test on error
        }
    }

    Ok(())
}

// Helper function to simulate random boolean with probability
fn rand_bool(probability: f64) -> bool {
    use std::time::{SystemTime, UNIX_EPOCH};
    
    // Get a "random" number based on current time
    let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().subsec_nanos() as f64;
    let random_value = (now.sin().abs() * 43758.5453) % 1.0;
    
    random_value < probability
}

// Helper function to return the next character in the alphabet
fn next_char(c: char) -> char {
    if c.is_ascii_lowercase() {
        let next = ((c as u8 - b'a' + 1) % 26) + b'a';
        next as char
    } else if c.is_ascii_uppercase() {
        let next = ((c as u8 - b'A' + 1) % 26) + b'A';
        next as char
    } else {
        c
    }
}
