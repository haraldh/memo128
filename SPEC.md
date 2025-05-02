**Specification: 128-bit Number + 7-bit Checksum to 3 Natural Language Sentences Encoder/Decoder**

* **Version:** 6.2
* **Date:** April 30, 2025
* **Author:** Gemini AI (based on user requirements)

**1. Introduction**

This document specifies the requirements for a software module capable of reversibly encoding a 128-bit unsigned integer, plus a 7-bit validation checksum, into a sequence of **three structured sentences** using natural language phrasing, and decoding them back to the original 128-bit integer after checksum verification. Each sentence corresponds to one of three generated sets of five story components (`Character`, `Setting`, `Action`, `Object/Theme`, `Outcome`), using predefined dictionaries containing potentially multi-word phrases with spaces. The system uses 135 bits ($128 \text{ data} + 7 \text{ checksum}$) which exactly matches the encoding capacity of the 3-sentence structure ($3 \times 45 \text{ bits}$). The resulting sentences are intended as mnemonic aids, forming mini-story fragments whose memorability depends significantly on the quality and thematic consistency of the dictionary content. Compatibility with this specified technical logic is the primary goal.

**2. Core Concept**

1.  A **7-bit checksum** is calculated from the initial 128 bits of data.
2.  The 128 data bits and 7 checksum bits are concatenated -> **135-bit sequence `N`**.
3.  This 135-bit sequence matches the capacity for encoding using **three full sets** of the specified components (Character: 10b, Setting: 10b, Action: 8b, Object: 9b, Outcome: 8b => 45 bits/set). $3 \times 45 = 135$ bits.
4.  The 135 bits are divided into **three 45-bit chunks**.
5.  Each 45-bit chunk is subdivided (10, 10, 8, 9, 8 bits) to yield indices for one set of components (Chunk 1 -> C1, S1, A1, O1, K1; etc.).
6.  The 5 elements derived from each chunk are looked up in component-specific dictionaries. These elements can be multi-word phrases containing spaces. They are assembled (space-separated) into **one sentence** for that chunk.
7.  The final encoded output is the ordered list of these **3 sentences**.
8.  Decoding reverses this: parses 3 sentences by identifying the 5 constituent dictionary phrases within each, performs reverse lookups, reconstructs three 45-bit chunks, combines them (135 bits), separates checksum, verifies checksum, and returns the 128 data bits.

**3. Data Structures and Formats**

**3.1. Input/Output Formats**

* **Encoding Input:** A 32-character hexadecimal string (128 bits data).
* **Encoding Output:** An ordered list or array containing exactly **3** strings (sentences). Each sentence is formed by concatenating 5 phrases (looked up from dictionaries) separated by single spaces.
    * **Sentence Structure:** `CharacterPhrase SettingPhrase ActionPhrase ObjectPhrase OutcomePhrase`
    * **Example Sentence (Illustrative Content):** `"a clever fox in the dark forest discovered a hidden map and found peace"` (Note: dictionary entries like "a clever fox" or "in the dark forest" contain spaces).
* **Decoding Input:** An ordered list or array of 3 sentence strings.
* **Decoding Output:** A 32-character, lowercase hexadecimal string (128 bits data), zero-padded. (Output only if checksum is valid).

**3.2. Bit Numbering and Order**

* Bits numbered 1 (MSB) to 128 (LSB) for data. Checksum appended.
* Requires handling up to 135-bit integers.

**3.3. Checksum Calculation**

* Take 128 bits (16 bytes) data. Calculate SHA256 hash. Use the **first 7 bits** (MSB) as checksum.

**3.4. Data+Checksum Sequence**

* Concatenate: `Data (128) | Checksum (7)` -> 135 bits `N`. No padding needed.

**3.5. Chunk Structure (135 bits)**

* **Chunk 1:** Bits 1-45 -> Sentence 1 (C1, S1, A1, O1, K1)
* **Chunk 2:** Bits 46-90 -> Sentence 2 (C2, S2, A2, O2, K2)
* **Chunk 3:** Bits 91-135 -> Sentence 3 (C3, S3, A3, O3, K3)

**3.6. Component Structure and Bit Allocation within 45-bit Chunks**

Each 45-bit chunk is allocated sequentially (MSB first):
* **Character Index:** 10 bits (0-1023)
* **Setting Index:** 10 bits (0-1023)
* **Action Index:** 8 bits (0-255)
* **Object Index:** 9 bits (0-511)
* **Outcome Index:** 8 bits (0-255)
* *Total: 45 bits.*

**3.7. Dictionary Files**

Five dictionary files are required.

* **File Naming and Sizes:**
    * `character_10bit.txt`: 1024 entries
    * `setting_10bit.txt`: 1024 entries
    * `action_8bit.txt`: 256 entries
    * `object_9bit.txt`: 512 entries
    * `outcome_8bit.txt`: 256 entries
* **File Format:** Plain text, UTF-8 recommended.
* **Entry Format:**
    * One unique entry per line (0-based index). Trim whitespace from ends.
    * **Entries MAY contain internal spaces** (e.g., "a clever fox"). **No underscores** should be used solely for parsing purposes.
    * Exactly the specified number of unique, non-empty lines per file.
* **Content Guidance (Recommended):** (Same as Version 6.1 - guides creation of meaningful phrases)
    * `character_10bit.txt`: Character descriptions (e.g., "a clever fox", "the lost astronaut").
    * `setting_10bit.txt`: Locations, environments, contexts (e.g., "in a dark forest", "on the moon base").
    * `action_8bit.txt`: Verbs or actions (e.g., "discovered", "carefully built").
    * `object_9bit.txt`: Objects, concepts, themes (e.g., "a hidden map", "infinite energy").
    * `outcome_8bit.txt`: Result/consequence phrases (e.g., "and found peace", "but lost the key").

**3.8. Sentence Assembly Pattern**

Each sentence corresponds to the components derived from one chunk:

* **Sentence 1:** `C1 S1 A1 O1 K1` (where C1, S1 etc. are the looked-up phrases)
* **Sentence 2:** `C2 S2 A2 O2 K2`
* **Sentence 3:** `C3 S3 A3 O3 K3`
* Phrases are joined by a single space character.

**4. Encoding Process (Algorithm)**

1.  **Input:** Receive 32-char hex string `hex_input`. Validate.
2.  **Convert to Bytes:** Convert hex to 16 bytes `data_bytes`.
3.  **Calculate Checksum:** Compute 7-bit `checksum_bits`.
4.  **Convert to Integer:** Convert `data_bytes` to 128-bit `data_num`.
5.  **Combine:** Create 135-bit `N = (data_num << 7) | checksum_bits`.
6.  **Initialize:** `output_sentences = []`, `current_bit_start = 1`.
7.  **Process 3 Chunks (Loop chunk_idx from 1 to 3):**
    a.  Extract 45 bits from `N` -> `chunk_value`.
    b.  Extract indices (10, 10, 8, 9, 8 bits): `idx_c`, `idx_s`, `idx_a`, `idx_o`, `idx_k`.
    c.  Lookup phrases `phrase_c, phrase_s, phrase_a, phrase_o, phrase_k` from respective dictionaries.
    d.  **Assemble Sentence:** Create sentence string: `f"{phrase_c} {phrase_s} {phrase_a} {phrase_o} {phrase_k}"`.
    e.  **Store Sentence:** Append sentence to `output_sentences`.
    f.  `current_bit_start += 45`.
8.  **Output:** Return `output_sentences` list (3 strings).

**5. Decoding Process (Algorithm)**

1.  **Input:** Receive list `input_sentences` (3 strings). Validate length is 3.
2.  **Initialize:** `reconstructed_135_num = 0`.
3.  **Prepare Reverse Lookups:** Ensure efficient reverse maps (phrase -> index) for all 5 dictionaries.
4.  **Process 3 Sentences (Loop i from 0 to 2):**
    a.  Get sentence string `s = input_sentences[i]`. Trim whitespace.
    b.  **Parse Sentence:** This step is now complex. The implementation must determine the original five dictionary phrases (`phrase_c`, `phrase_s`, `phrase_a`, `phrase_o`, `phrase_k`) that were concatenated with spaces to form `s`.
        i.  It must compare the start of the (remaining) sentence against all entries in the expected dictionary (first `character_list`, then `setting_list`, etc.).
        ii. It must find the **unique sequence** of one phrase from each of the 5 required dictionaries that exactly reconstructs the input sentence `s` when joined by single spaces.
        iii. Strategies like prioritizing longer matches might be needed if dictionary entries are prefixes of others (e.g., "King" vs "King Arthur"). The method must be deterministic.
        iv. If a unique, valid sequence of 5 phrases cannot be identified, report a parsing error.
    c.  **Reverse Lookup Indices:** Once the 5 phrases are identified, find their corresponding indices `idx_c, idx_s, idx_a, idx_o, idx_k` using the reverse lookup maps. Handle lookup errors if a parsed phrase isn't in the expected dictionary.
    d.  **Reconstruct Chunk Value:** Calculate the 45-bit `chunk_value`:
        `chunk_value = (idx_c << 35) | (idx_s << 25) | (idx_a << 17) | (idx_o << 8) | idx_k`
    e.  **Append to Number:** Append `chunk_value`:
        `reconstructed_135_num = (reconstructed_135_num << 45) | chunk_value`
5.  **Separate Data and Checksum:** (From `reconstructed_135_num`)
    * `checksum_bits_decoded = reconstructed_135_num & 0x7F` (Mask last 7 bits).
    * `data_num_decoded = reconstructed_135_num >> 7`.
6.  **Verify Checksum:**
    a.  Convert `data_num_decoded` (128 bits) to 16 bytes `data_bytes_decoded`.
    b.  Calculate `sha256(data_bytes_decoded)`. Get first 7 bits `checksum_bits_calculated`.
    c.  Compare decoded and calculated checksums. If mismatch, report Checksum Error and **stop**.
7.  **Format Output:** Convert `data_num_decoded` into a 32-character hex string, lowercase, zero-padded.
8.  **Output:** Return the formatted hex string.

**6. Dictionary Handling**

1.  **Loading:** Load the 5 specified dictionary files.
2.  **Error Handling:** Handle file errors, size mismatches, duplicates.
3.  **Format:** Trim whitespace from ends of lines, ignore empty lines, ensure uniqueness. **Entries may contain spaces.**
4.  **Reverse Lookup:** Implement efficient reverse lookup (phrase -> index).

**7. Error Handling**

Handle and report errors including:
* Invalid input format/structure.
* Dictionary file issues.
* **Sentence parsing errors (failure to uniquely identify the 5 constituent dictionary phrases).**
* Phrase not found in dictionary during reverse lookup.
* Checksum verification failure.

**8. Compatibility Requirements**

* Must be compatible with this specification regarding checksum (7 bits), bit allocation, dictionary usage, and sentence assembly/parsing logic.

**9. Implementation Notes**

* **128-bit Integers:** Requires support for up to 135-bit integer operations.
* **Checksum Library:** Requires SHA256.
* **Character Encoding:** Use UTF-8.
* **Decoding Complexity:** Note that the sentence parsing logic during decoding (Step 5b) is significantly more complex than simple string splitting due to dictionary entries potentially containing spaces. Careful implementation is required to ensure correct and reasonably efficient parsing.
