# Изменения для Bitcoin Recovery

Этот документ описывает ключевые изменения в `src/main.rs` для поддержки Bitcoin вместо Ethereum.

## Краткая сводка изменений:

```
Ethereum → Bitcoin
- Keccak256 → SHA256 + RIPEMD160
- ETH address (20 bytes) → BTC addresses (3 types: 25 + 25 + 21 = 71 bytes)
- m/44'/60'/0'/0/0 → m/44'/0'/0'/0/0 (P2PKH)
                   → m/49'/0'/0'/0/0 (P2SH)
                   → m/84'/0'/0'/0/0 (P2WPKH)
- Hex encoding → Base58 + Bech32 encoding
```

## Изменения в `src/main.rs`:

### 1. Заголовок файла (строки 1-10):

```rust
// Bitcoin BIP39 Recovery Tool - GPU Worker Client
// GPU генерирует 3 типа BTC адресов (P2PKH, P2SH, P2WPKH), CPU проверяет в БД

mod db_loader;

use db_loader::Database;
use std::collections::HashMap;
use std::fs;
use ocl::{flags, ProQue, Buffer};
use serde::Deserialize;
```

### 2. Константы (строки 13-16):

```rust
// === Конфигурация ===
const WORK_SERVER_URL: &str = "http://90.156.225.121:3000";
const WORK_SERVER_SECRET: &str = "15a172308d70dede515f9eecc78eaea9345b419581d0361220313d938631b12d";
const DATABASE_PATH: &str = "btc_addresses_db";  // ← ИЗМЕНЕНО
const BATCH_SIZE: usize = 5000000;
```

### 3. Функция `log_solution` (строка 76):

```rust
fn log_solution(offset: u128, mnemonic: String, btc_addresses: Vec<String>) -> Result<(), Box<dyn std::error::Error>> {
    let mut json_body = HashMap::new();
    json_body.insert("mnemonic", mnemonic.clone());
    json_body.insert("btc_p2pkh", btc_addresses[0].clone());   // Legacy (1...)
    json_body.insert("btc_p2sh", btc_addresses[1].clone());    // SegWit (3...)
    json_body.insert("btc_bech32", btc_addresses[2].clone());  // Native SegWit (bc1...)
    json_body.insert("offset", offset.to_string());
    json_body.insert("secret", WORK_SERVER_SECRET.to_string());

    let client = reqwest::blocking::Client::new();
    let url = format!("{}/mnemonic", WORK_SERVER_URL);
    client.post(&url).json(&json_body).send()?;

    println!("\n🎉🎉🎉 РЕШЕНИЕ НАЙДЕНО! 🎉🎉🎉");
    println!("Мнемоника: {}", mnemonic);
    println!("P2PKH (1...): {}", btc_addresses[0]);
    println!("P2SH (3...): {}", btc_addresses[1]);
    println!("Bech32 (bc1...): {}", btc_addresses[2]);
    println!("Offset: {}", offset);

    Ok(())
}
```

### 4. OpenCL файлы (строка 100-117):

```rust
fn build_kernel_source() -> Result<String, Box<dyn std::error::Error>> {
    let cl_dir = "cl/";

    let files = vec![
        "common.cl",
        "sha2.cl",
        "pbkdf2_bip39.cl",
        // ❌ УБРАТЬ: "keccak256.cl",        // НЕ нужен для Bitcoin
        // ❌ УБРАТЬ: "eth_address.cl",      // НЕ нужен для Bitcoin
        "secp256k1_common.cl",
        "secp256k1_field.cl",
        "secp256k1_group.cl",
        "secp256k1_scalar.cl",
        "secp256k1_prec.cl",
        "secp256k1.cl",
        "ripemd.cl",
        "address.cl",
        "btc_address.cl",           // ✅ НОВЫЙ файл!
        "mnemonic_constants.cl",
        "mnemonic_generator.cl",
    ];
    // ... rest of the function
}
```

### 5. GPU Kernel (строка 136-268):

**ЗАМЕНИТЬ** `generate_eth_addresses` на `generate_btc_addresses`:

```rust
source.push_str(r#"
// === Bitcoin Address Generator Kernel ===
// Генерирует 3 типа адресов: P2PKH (1...), P2SH (3...), P2WPKH (bc1...)

__kernel void generate_btc_addresses(
    __global uchar *result_addresses,     // Output: 71 байт на комбинацию (25+25+21)
    __global uchar *result_mnemonics,     // Output: мнемоники (192 bytes)
    const ulong start_offset,
    const uint batch_size
) {
    uint gid = get_global_id(0);
    if (gid >= batch_size) return;

    ulong current_offset = start_offset + gid;

    // Декодируем offset в индексы слов (как в ETH версии)
    uint last_3_bits = (uint)(current_offset % 8UL);
    ulong temp = current_offset / 8UL;
    uint w22_idx = (uint)(temp % 2048UL);
    temp = temp / 2048UL;
    uint w21_idx = (uint)(temp % 2048UL);
    uint w20_idx = (uint)((temp / 2048UL) % 2048UL);

    // Build word indices (20 известных + 3 неизвестных)
    uint word_indices[24];
    word_indices[0] = 1761;   // switch
    word_indices[1] = 1263;   // over
    // ... (остальные 18 слов)
    word_indices[20] = w20_idx;
    word_indices[21] = w21_idx;
    word_indices[22] = w22_idx;

    // Calculate checksum для 24-го слова (как в ETH версии)
    uchar entropy[32];
    for(int i = 0; i < 32; i++) entropy[i] = 0;

    uint bit_pos = 0;
    for(int w = 0; w < 23; w++) {
        uint word_val = word_indices[w];
        for(int b = 10; b >= 0; b--) {
            uint bit = (word_val >> b) & 1;
            uint byte_idx = bit_pos / 8;
            uint bit_idx = 7 - (bit_pos % 8);
            if(byte_idx < 32) {
                entropy[byte_idx] |= (bit << bit_idx);
            }
            bit_pos++;
        }
    }

    entropy[31] = (entropy[31] & 0xF8) | last_3_bits;

    uchar hash[32];
    sha256_bytes(entropy, 32, hash);
    uchar checksum = hash[0];
    uint w23_idx = (last_3_bits << 8) | checksum;
    word_indices[23] = w23_idx;

    // Build mnemonic
    uchar mnemonic[192];
    for(int i = 0; i < 192; i++) mnemonic[i] = 0;

    int pos = 0;
    for(int w = 0; w < 24; w++) {
        uint word_idx = word_indices[w];
        for(int c = 0; c < 8 && words[word_idx][c] != '\0'; c++) {
            mnemonic[pos++] = words[word_idx][c];
        }
        if(w < 23) mnemonic[pos++] = ' ';
    }
    uint mnemonic_len = pos;

    // PBKDF2 → seed
    uchar seed[64];
    for(int i = 0; i < 64; i++) seed[i] = 0;
    mnemonic_to_seed(mnemonic, mnemonic_len, seed);

    // ✅ НОВОЕ: Генерируем ВСЕ 3 типа BTC адресов
    uchar all_btc_addresses[71];  // 25 + 25 + 21 bytes
    for(int i = 0; i < 71; i++) all_btc_addresses[i] = 0;

    derive_all_btc_addresses(seed, all_btc_addresses);

    // Write results
    for(int i = 0; i < 71; i++) {
        result_addresses[gid * 71 + i] = all_btc_addresses[i];
    }

    for(int i = 0; i < 192; i++) {
        result_mnemonics[gid * 192 + i] = mnemonic[i];
    }
}
"#);
```

### 6. GPU Worker Function (строка 318-365):

**ИЗМЕНИТЬ** размер буфера и чтение результатов:

```rust
// Вместо 8 байт (ETH addr suffix), теперь 71 байт (3 BTC адреса)
let result_addresses: Buffer<u8> = pro_que.buffer_builder()
    .len(batch_size * 71)  // ← ИЗМЕНЕНО: 71 байт вместо 8
    .flags(flags::MEM_WRITE_ONLY)
    .build()?;

let result_mnemonics: Buffer<u8> = pro_que.buffer_builder()
    .len(batch_size * 192)
    .flags(flags::MEM_WRITE_ONLY)
    .build()?;
```

### 7. CPU-сторона: Декодирование адресов (строка 363-430):

**ЗАМЕНИТЬ** чтение и проверку адресов:

```rust
// Читаем результаты с GPU
let mut addresses_bytes = vec![0u8; chunk_size as usize * 71];
result_addresses.read(&addresses_bytes).enq()?;

let mut mnemonics_data = vec![0u8; chunk_size as usize * 192];
result_mnemonics.read(&mut mnemonics_data).enq()?;

// CPU lookup с Base58/Bech32 декодированием
print!("   🔍 CPU lookup (Base58/Bech32)...");
let mut found_count = 0;

for i in 0..chunk_size as usize {
    let addr_bytes = &addresses_bytes[i * 71..(i + 1) * 71];

    // Декодируем 3 типа адресов
    let p2pkh_bytes = &addr_bytes[0..25];    // Legacy
    let p2sh_bytes = &addr_bytes[25..50];    // SegWit
    let p2wpkh_bytes = &addr_bytes[50..71];  // Native SegWit

    // Base58 для P2PKH и P2SH
    let p2pkh_addr = bs58::encode(p2pkh_bytes).into_string();
    let p2sh_addr = bs58::encode(p2sh_bytes).into_string();

    // Bech32 для P2WPKH
    use bech32::{ToBase32, Variant};
    let p2wpkh_addr = bech32::encode(
        "bc",  // mainnet
        p2wpkh_bytes[1..].to_base32(),  // skip version byte
        Variant::Bech32
    ).unwrap_or_default();

    // Проверка в БД (по hash160 - байты 1-20)
    let hash160_p2pkh = &p2pkh_bytes[1..21];
    let hash160_p2sh = &p2sh_bytes[1..21];
    let hash160_p2wpkh = &p2wpkh_bytes[1..21];

    let found = db.lookup_bitcoin_address(hash160_p2pkh) ||
                db.lookup_bitcoin_address(hash160_p2sh) ||
                db.lookup_bitcoin_address(hash160_p2wpkh);

    if found {
        let mnemonic_start = i * 192;
        let mnemonic_bytes = &mnemonics_data[mnemonic_start..mnemonic_start + 192];
        let mnemonic = String::from_utf8_lossy(mnemonic_bytes);
        let mnemonic_clean = mnemonic.trim_matches('\0').trim();

        println!("\n\n🎉🎉🎉 НАЙДЕНО! 🎉🎉🎉");
        println!("Мнемоника: {}", mnemonic_clean);
        println!("P2PKH: {}", p2pkh_addr);
        println!("P2SH: {}", p2sh_addr);
        println!("Bech32: {}", p2wpkh_addr);

        let addrs = vec![p2pkh_addr, p2sh_addr, p2wpkh_addr];
        if let Err(e) = log_solution(work.offset_for_server + i as u128,
                                      mnemonic_clean.to_string(), addrs) {
            eprintln!("⚠️ Ошибка: {}", e);
        }
        found_count += 1;
    }
}
```

### 8. Main function (строка 442-489):

```rust
fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Bitcoin BIP39 Recovery - GPU Worker ===\n");

    println!("Задача: 24-словная BIP39 мнемоника для Bitcoin");
    println!("  Известно: первые 20 слов");
    println!("  Неизвестно: последние 4 слова");
    println!("  Генерируем 3 типа адресов:");
    println!("    - P2PKH (Legacy, starts with '1')");
    println!("    - P2SH-P2WPKH (SegWit, starts with '3')");
    println!("    - P2WPKH (Native SegWit, starts with 'bc1')\n");

    // ... rest stays the same
}
```

## Следующий шаг:

После этих изменений обновите `db_loader.rs` для поддержки Bitcoin БД формата (hash160 вместо addr_suffix).
