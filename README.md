# Memo128

A Rust library and CLI tool for encoding 128-bit numbers as memorable natural language sentences.

## Overview

Memo128 converts cryptographic keys, blockchain addresses, and other 128-bit values into easy-to-remember sentences. It adds a 7-bit checksum for error detection and uses five dictionaries to create structured sentences that form mini-stories.

Each 128-bit value (plus 7-bit checksum) is encoded as three sentences, with each sentence containing:
- Character (10 bits)
- Setting (10 bits)
- Action (8 bits)
- Object (9 bits)
- Outcome (8 bits)

## Installation

```bash
cargo install --path .
```

## Usage

### Encoding a 128-bit hex value

```bash
cargo run -- encode 0123456789abcdef0123456789abcdef
```

This will output three sentences representing the encoded value.

```
Sentence 1: a brave mouse inside a cosmic cathedral disconnected a question of time but it was already too late
Sentence 2: a worried parent within the cosmic algorithm accepted a mathematical impossibility as code predicted
Sentence 3: the fjord elder inside a particle accelerator stole a reality glitch rebooting the system
```

### Decoding sentences back to the original value

```bash
cargo run -- decode \
  "a brave mouse inside a cosmic cathedral disconnected a question of time but it was already too late" \
  "a worried parent within the cosmic algorithm accepted a mathematical impossibility as code predicted" \
  "the fjord elder inside a particle accelerator stole a reality glitch rebooting the system"
```

Make sure to quote each sentence and provide all three sentences in the correct order.

## Requirements

- Rust 1.70+
- Five dictionary files in the current directory:
  - `character_10bit.txt` (1024 entries)
  - `setting_10bit.txt` (1024 entries)
  - `action_8bit.txt` (256 entries)
  - `object_9bit.txt` (512 entries)
  - `outcome_8bit.txt` (256 entries)

## How It Works

1. Input a 32-character hexadecimal string (128 bits)
2. Calculate a 7-bit checksum using SHA-256
3. Combine into a 135-bit sequence
4. Split into three 45-bit chunks
5. Map each chunk to story components using dictionaries
6. Assemble three sentences from the components

For full details, see [SPEC.md](SPEC.md).

## Development

```bash
# Build the project
cargo build

# Run tests
cargo test

# Format code
cargo fmt

# Run linter
cargo clippy
```
