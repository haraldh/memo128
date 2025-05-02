use memo128::{Memo128, fuzzy::FuzzyMemo128};

#[test]
fn test_fuzzy_decode() -> Result<(), Box<dyn std::error::Error>> {
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
            
            // Print all results
            for (result_idx, result) in results.iter().enumerate() {
                println!("  Result {}: {}", result_idx + 1, result);
            }
        },
        Err(e) => {
            eprintln!("Error: {}", e);
            return Err(e.into());
        }
    }
    
    Ok(())
}