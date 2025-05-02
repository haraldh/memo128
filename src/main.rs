use memo128::Memo128;
use memo128::fuzzy::FuzzyMemo128;

fn print_usage() {
    println!("Usage:");
    println!("  encode <hex_string>   - Encode a 32-character hex string to 3 sentences");
    println!("  decode \"<s1>\" \"<s2>\" \"<s3>\" - Decode 3 sentences back to a hex string");
    println!("  fuzzy-decode [--max-distance=N] \"<s1>\" \"<s2>\" \"<s3>\" - Fuzzy decode with Levenshtein distance");
    println!("");
    println!("Options:");
    println!("  --max-distance=N   - Maximum Levenshtein distance for fuzzy matching (default: 2)");
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();

    if args.len() < 2 {
        print_usage();
        return Ok(());
    }

    let memo128 = Memo128::new()?;

    match args[1].as_str() {
        "encode" => {
            if args.len() != 3 {
                println!("Error: encode command requires a 32-character hex string");
                print_usage();
                return Ok(());
            }

            match memo128.encode(&args[2]) {
                Ok(sentences) => {
                    for (i, sentence) in sentences.iter().enumerate() {
                        println!("Sentence {}: {}", i + 1, sentence);
                    }
                }
                Err(e) => println!("Error: {}", e),
            }
        }
        "decode" => {
            if args.len() != 5 {
                println!("Error: decode command requires exactly 3 sentences");
                print_usage();
                return Ok(());
            }

            let sentences = vec![args[2].clone(), args[3].clone(), args[4].clone()];

            match memo128.decode(&sentences) {
                Ok(hex) => println!("{}", hex),
                Err(e) => println!("Error: {}", e),
            }
        }
        "fuzzy-decode" => {
            // Parse arguments and options
            let mut max_distance = 3; // Default value
            let mut sentence_start_idx = 2;
            
            // Check for --max-distance option
            for (i, arg) in args.iter().enumerate().skip(2) {
                if arg.starts_with("--max-distance=") {
                    let value = &arg["--max-distance=".len()..];
                    match value.parse::<usize>() {
                        Ok(dist) => {
                            max_distance = dist;
                            sentence_start_idx = i + 1;
                        }
                        Err(_) => {
                            println!("Error: Invalid value for --max-distance");
                            print_usage();
                            return Ok(());
                        }
                    }
                    break;
                }
            }
            
            // Check if we have exactly 3 sentences
            if args.len() < sentence_start_idx + 3 {
                println!("Error: fuzzy-decode command requires exactly 3 sentences");
                print_usage();
                return Ok(());
            }
            
            let sentences = vec![
                args[sentence_start_idx].clone(),
                args[sentence_start_idx + 1].clone(),
                args[sentence_start_idx + 2].clone(),
            ];
            
            // Create the fuzzy decoder with the specified max distance
            let fuzzy_memo128 = FuzzyMemo128::new(max_distance)?;
            
            // Perform fuzzy decoding
            match fuzzy_memo128.fuzzy_decode(&sentences) {
                Ok(results) => {
                    if results.is_empty() {
                        println!("No valid matches found.");
                    } else {
                        println!("Found {} possible matches:", results.len());
                        for (i, hex) in results.iter().enumerate() {
                            println!("Match {}: {} - {}", i + 1, hex, memo128.encode(hex).unwrap().join(". "));
                        }
                    }
                }
                Err(e) => println!("Error: {}", e),
            }
        }
        _ => {
            println!("Error: Unknown command '{}'", args[1]);
            print_usage();
        }
    }

    Ok(())
}
