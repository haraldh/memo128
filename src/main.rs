use memo128::Memo128;

fn print_usage() {
    println!("Usage:");
    println!("  encode <hex_string>   - Encode a 32-character hex string to 3 sentences");
    println!("  decode \"<s1>\" \"<s2>\" \"<s3>\" - Decode 3 sentences back to a hex string");
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
        _ => {
            println!("Error: Unknown command '{}'", args[1]);
            print_usage();
        }
    }

    Ok(())
}
