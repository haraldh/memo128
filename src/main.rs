use clap::{Parser, Subcommand};
use memo128::fuzzy::FuzzyMemo128;
use memo128::Memo128;

#[derive(Parser)]
#[command(name = "memo128")]
#[command(about = "Encode/decode binary data as memorable sentences", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Encode a 32-character hex string to 3 sentences
    Encode {
        /// The 32-character hex string to encode
        hex_string: String,
    },
    /// Decode 3 sentences back to a hex string
    Decode {
        /// The three sentences to decode
        sentences: Vec<String>,
    },
    /// Fuzzy decode with Levenshtein distance
    #[command(name = "fuzzy-decode")]
    FuzzyDecode {
        /// Maximum Levenshtein distance for fuzzy matching
        #[arg(long, default_value_t = 3)]
        max_distance: usize,

        /// The three sentences to fuzzy decode
        sentences: Vec<String>,
    },
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();
    let memo128 = Memo128::new()?;

    match cli.command {
        Commands::Encode { hex_string } => match memo128.encode(&hex_string) {
            Ok(sentences) => {
                for (i, sentence) in sentences.iter().enumerate() {
                    println!("Sentence {}: {}", i + 1, sentence);
                }
            }
            Err(e) => println!("Error: {}", e),
        },
        Commands::Decode { sentences } => {
            if sentences.len() != 3 {
                println!("Error: decode command requires exactly 3 sentences");
                return Ok(());
            }

            match memo128.decode(&sentences) {
                Ok(hex) => println!("{}", hex),
                Err(e) => println!("Error: {}", e),
            }
        }
        Commands::FuzzyDecode {
            max_distance,
            sentences,
        } => {
            if sentences.len() != 3 {
                println!("Error: fuzzy-decode command requires exactly 3 sentences");
                return Ok(());
            }

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
                            println!(
                                "Match {}: {} - {}",
                                i + 1,
                                hex,
                                memo128.encode(hex).unwrap().join(". ")
                            );
                        }
                    }
                }
                Err(e) => println!("Error: {}", e),
            }
        }
    }

    Ok(())
}
