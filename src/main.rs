// Bitcoin BIP39 Recovery Tool - GPU Worker Client
// GPU генерирует 3 типа BTC адресов (P2PKH/P2SH/P2WPKH), CPU проверяет в БД (без загрузки БД в GPU)

mod db_loader;

use db_loader::Database;
use std::collections::HashMap;
use std::fs;
use ocl::{flags, ProQue, Buffer};
use serde::Deserialize;

// === Конфигурация ===
const WORK_SERVER_URL: &str = "http://90.156.225.121:3000";
const WORK_SERVER_SECRET: &str = "15a172308d70dede515f9eecc78eaea9345b419581d0361220313d938631b12d";
const DATABASE_PATH: &str = "btc-20200101-to-20250201.db";  // Bitcoin DB
const BATCH_SIZE: usize = 5000000; // 1M per GPU batch

// Известные 20 слов (4 неизвестных: позиции 20, 21, 22, 23)
// Seed: switch real throw crush baby oven jazz steak donate carbon fever vague yellow three control flavor sugar salad base false ??? ??? ??? ???
// Total combinations: 2048³ × 8 = 68,719,476,736
const KNOWN_WORDS: [&str; 20] = [
    "switch", "real", "throw", "crush", "baby",      // positions 0-4
    "oven", "jazz", "steak", "donate", "carbon",     // positions 5-9
    "fever", "vague", "yellow", "three", "control",  // positions 10-14
    "flavor", "sugar", "salad", "base", "false"      // positions 15-19
];

// === API структуры для работы с сервером ===

#[derive(Deserialize, Debug, Default)]
struct WorkResponse {
    #[serde(default)]
    indices: Vec<u128>,
    #[serde(default)]
    offset: u128,
    #[serde(default = "default_batch_size")]
    batch_size: u64,
}

fn default_batch_size() -> u64 {
    BATCH_SIZE as u64
}

struct Work {
    start_offset: u64,
    batch_size: u64,
    offset_for_server: u128,
}

// === Функции для работы с оркестратором ===

fn get_work() -> Result<Work, Box<dyn std::error::Error>> {
    let url = format!("{}/work?secret={}", WORK_SERVER_URL, WORK_SERVER_SECRET);
    let response = reqwest::blocking::get(&url)?;
    let work_response: WorkResponse = response.json()?;

    let start_offset = work_response.offset;

    Ok(Work {
        start_offset: start_offset as u64,
        batch_size: work_response.batch_size,
        offset_for_server: work_response.offset,
    })
}

fn log_work_complete(offset: u128) -> Result<(), Box<dyn std::error::Error>> {
    let mut json_body = HashMap::new();
    json_body.insert("offset", offset.to_string());
    json_body.insert("secret", WORK_SERVER_SECRET.to_string());

    let client = reqwest::blocking::Client::new();
    let url = format!("{}/work", WORK_SERVER_URL);
    client.post(&url).json(&json_body).send()?;

    Ok(())
}

fn log_solution(offset: u128, mnemonic: String, btc_addresses: Vec<String>) -> Result<(), Box<dyn std::error::Error>> {
    let mut json_body = HashMap::new();
    json_body.insert("mnemonic", mnemonic.clone());
    json_body.insert("btc_p2pkh", btc_addresses[0].clone());
    json_body.insert("btc_p2sh", btc_addresses[1].clone());
    json_body.insert("btc_bech32", btc_addresses[2].clone());
    json_body.insert("offset", offset.to_string());
    json_body.insert("secret", WORK_SERVER_SECRET.to_string());

    let client = reqwest::blocking::Client::new();
    let url = format!("{}/mnemonic", WORK_SERVER_URL);
    client.post(&url).json(&json_body).send()?;

    println!("\n🎉🎉🎉 РЕШЕНИЕ НАЙДЕНО! 🎉🎉🎉");
    println!("Мнемоника: {}", mnemonic);
    println!("P2PKH:  {}", btc_addresses[0]);
    println!("P2SH:   {}", btc_addresses[1]);
    println!("Bech32: {}", btc_addresses[2]);
    println!("Offset: {}", offset);

    Ok(())
}

// === OpenCL Kernel Builder ===

fn build_kernel_source() -> Result<String, Box<dyn std::error::Error>> {
    let cl_dir = "cl/";

    let files = vec![
        "common.cl",
        "sha2.cl",
        "pbkdf2_bip39.cl",
        // "keccak256.cl",  // Not needed for Bitcoin
        "secp256k1_common.cl",
        "secp256k1_field.cl",
        "secp256k1_group.cl",
        "secp256k1_scalar.cl",
        "secp256k1_prec.cl",
        "secp256k1.cl",
        "ripemd.cl",
        "address.cl",
        "btc_address.cl",  // Bitcoin address derivation
        "mnemonic_constants.cl",
        "mnemonic_generator.cl",
        // "bip39_checksum.cl",  // Не используется, код встроен в kernel ниже
    ];

    let mut source = String::new();

    for file in files {
        let path = format!("{}{}", cl_dir, file);
        match fs::read_to_string(&path) {
            Ok(content) => {
                source.push_str(&format!("\n// === {} ===\n", file));
                source.push_str(&content);
            }
            Err(e) => {
                eprintln!("⚠️  Warning: Could not read {}: {}", path, e);
            }
        }
    }

    // Bitcoin Address Generator Kernel for 20 known words + 4 unknown (positions 20, 21, 22, 23)
    // Total combinations: 2048³ × 8 = 68,719,476,736
    source.push_str(r#"
__kernel void generate_btc_addresses(
    __global uchar *result_addresses,     // Output: 71 bytes per combo (25+25+21)
    __global uchar *result_mnemonics,     // Output: 192 bytes per mnemonic
    const ulong start_offset,
    const uint batch_size
) {
    uint gid = get_global_id(0);
    if (gid >= batch_size) return;

    ulong current_offset = start_offset + gid;

    // For 4 unknown words (positions 20, 21, 22, 23):
    // - Word 20: 2048 variants (11 bits)
    // - Word 21: 2048 variants (11 bits)
    // - Word 22: 2048 variants (11 bits)
    // - Word 23: calculated from checksum (3 bits entropy + 8 bits checksum)
    // Total: 2048³ × 8 = 68,719,476,736 combinations

    uint last_3_bits = (uint)(current_offset % 8UL);   // 0-7
    ulong temp = current_offset / 8UL;
    uint w22_idx = (uint)(temp % 2048UL);              // 0-2047
    temp = temp / 2048UL;
    uint w21_idx = (uint)(temp % 2048UL);              // 0-2047
    uint w20_idx = (uint)((temp / 2048UL) % 2048UL);   // 0-2047

    // Build word indices (20 known + 4 unknown at positions 20, 21, 22, 23)
    // Seed: switch real throw crush baby oven jazz steak donate carbon fever vague yellow three control flavor sugar salad base false ??? ??? ??? ???
    uint word_indices[24];
    word_indices[0] = 1761;   // switch
    word_indices[1] = 1431;   // real
    word_indices[2] = 1802;   // throw
    word_indices[3] = 424;    // crush
    word_indices[4] = 136;    // baby
    word_indices[5] = 1262;   // oven
    word_indices[6] = 955;    // jazz
    word_indices[7] = 1704;   // steak
    word_indices[8] = 520;    // donate
    word_indices[9] = 275;    // carbon
    word_indices[10] = 683;   // fever
    word_indices[11] = 1925;  // vague
    word_indices[12] = 2040;  // yellow
    word_indices[13] = 1800;  // three
    word_indices[14] = 379;   // control
    word_indices[15] = 709;   // flavor
    word_indices[16] = 1734;  // sugar
    word_indices[17] = 1522;  // salad
    word_indices[18] = 151;   // base
    word_indices[19] = 658;   // false
    word_indices[20] = w20_idx; // UNKNOWN - iterate
    word_indices[21] = w21_idx; // UNKNOWN - iterate
    word_indices[22] = w22_idx; // UNKNOWN - iterate
    // word_indices[23] calculated below from checksum

    // Calculate checksum word (word 23)
    // Pack 253 bits from first 23 words
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

    // Add last 3 bits to complete 256 bits of entropy
    entropy[31] = (entropy[31] & 0xF8) | last_3_bits;

    // SHA256 of entropy to get checksum
    uchar hash[32];
    for(int i = 0; i < 32; i++) hash[i] = 0;
    sha256_bytes(entropy, 32, hash);

    // Checksum = first 8 bits of hash
    uchar checksum = hash[0];

    // Word 23 = (last_3_bits << 8) | checksum
    uint w23_idx = (last_3_bits << 8) | checksum;
    word_indices[23] = w23_idx;

    // Build mnemonic string
    uchar mnemonic[192];
    for(int i = 0; i < 192; i++) mnemonic[i] = 0;

    int pos = 0;
    for(int w = 0; w < 24; w++) {
        uint word_idx = word_indices[w];
        for(int c = 0; c < 8 && words[word_idx][c] != 0; c++) {
            mnemonic[pos++] = words[word_idx][c];
        }
        if(w < 23) mnemonic[pos++] = ' ';
    }
    uint mnemonic_len = pos;

    // Convert mnemonic to seed
    uchar seed[64];
    for(int i = 0; i < 64; i++) seed[i] = 0;
    mnemonic_to_seed(mnemonic, mnemonic_len, seed);

    // === INLINE BTC ADDRESS DERIVATION ===
    // Using reusable variables to minimize GPU memory
    extended_private_key_t master;
    extended_private_key_t child1;
    extended_private_key_t child2;
    extended_public_key_t pub;
    
    new_master_from_seed(BITCOIN_MAINNET, seed, &master);

    // === 1. P2PKH (Legacy) - BIP44: m/44'/0'/0'/0/0 ===
    hardened_private_child_from_private(&master, &child1, 44);
    hardened_private_child_from_private(&child1, &child2, 0);
    hardened_private_child_from_private(&child2, &child1, 0);
    normal_private_child_from_private(&child1, &child2, 0);
    normal_private_child_from_private(&child2, &child1, 0);
    public_from_private(&child1, &pub);
    
    // Get hash160 for P2PKH
    uchar pubkey_hash[20];
    identifier_for_public_key(&pub, pubkey_hash);
    
    // Write P2PKH address (version 0x00 + hash160 + checksum)
    result_addresses[gid * 71 + 0] = 0x00;
    for(int i = 0; i < 20; i++) result_addresses[gid * 71 + 1 + i] = pubkey_hash[i];
    uchar addr_data[21];
    addr_data[0] = 0x00;
    for(int i = 0; i < 20; i++) addr_data[i+1] = pubkey_hash[i];
    uchar addr_checksum[32];
    sha256d(addr_data, 21, addr_checksum);
    for(int i = 0; i < 4; i++) result_addresses[gid * 71 + 21 + i] = addr_checksum[i];

    // === 2. P2SH-P2WPKH (SegWit) - BIP49: m/49'/0'/0'/0/0 ===
    hardened_private_child_from_private(&master, &child1, 49);
    hardened_private_child_from_private(&child1, &child2, 0);
    hardened_private_child_from_private(&child2, &child1, 0);
    normal_private_child_from_private(&child1, &child2, 0);
    normal_private_child_from_private(&child2, &child1, 0);
    public_from_private(&child1, &pub);
    identifier_for_public_key(&pub, pubkey_hash);
    
    // P2SH script: OP_0 <20-byte-hash>
    uchar script[22];
    script[0] = 0x00;
    script[1] = 0x14;
    for(int i = 0; i < 20; i++) script[i+2] = pubkey_hash[i];
    uchar script_hash[20];
    hash160(script, 22, script_hash);
    
    result_addresses[gid * 71 + 25] = 0x05;
    for(int i = 0; i < 20; i++) result_addresses[gid * 71 + 26 + i] = script_hash[i];
    addr_data[0] = 0x05;
    for(int i = 0; i < 20; i++) addr_data[i+1] = script_hash[i];
    sha256d(addr_data, 21, addr_checksum);
    for(int i = 0; i < 4; i++) result_addresses[gid * 71 + 46 + i] = addr_checksum[i];

    // === 3. P2WPKH (Native SegWit) - BIP84: m/84'/0'/0'/0/0 ===
    hardened_private_child_from_private(&master, &child1, 84);
    hardened_private_child_from_private(&child1, &child2, 0);
    hardened_private_child_from_private(&child2, &child1, 0);
    normal_private_child_from_private(&child1, &child2, 0);
    normal_private_child_from_private(&child2, &child1, 0);
    public_from_private(&child1, &pub);
    identifier_for_public_key(&pub, pubkey_hash);
    
    result_addresses[gid * 71 + 50] = 0x00;
    for(int i = 0; i < 20; i++) result_addresses[gid * 71 + 51 + i] = pubkey_hash[i];

    // Write mnemonic
    for(int i = 0; i < 192; i++) {
        result_mnemonics[gid * 192 + i] = mnemonic[i];
    }
}
"#);

    Ok(source)
}

fn run_gpu_worker(db: &Database) -> Result<(), Box<dyn std::error::Error>> {
    println!("\n🚀 Запуск GPU Worker...\n");

    println!("📚 Компиляция OpenCL kernel...");
    let kernel_source = build_kernel_source()?;
    println!("   Kernel source: {} KB", kernel_source.len() / 1024);

    use ocl::{Platform, Device, DeviceType};

    let platform = Platform::list()
        .into_iter()
        .find(|p| {
            p.name().unwrap_or_default().contains("NVIDIA") ||
            p.vendor().unwrap_or_default().contains("NVIDIA")
        })
        .or_else(|| Platform::list().into_iter().next())
        .ok_or("No OpenCL platform found")?;

    let device = Device::list(platform, Some(DeviceType::GPU))
        .ok()
        .and_then(|devices| devices.into_iter().next())
        .ok_or("No GPU device found")?;

    println!("📱 Выбрано устройство:");
    println!("   Platform: {}", platform.name()?);
    println!("   Device: {}", device.name()?);
    println!("   Type: GPU");

    // Save kernel source for debugging
    std::fs::write("kernel_debug.cl", &kernel_source).ok();
    println!("   Kernel saved to kernel_debug.cl");

    let pro_que = match ProQue::builder()
        .src(&kernel_source)
        .dims(1)
        .platform(platform)
        .device(device)
        .build() {
            Ok(pq) => pq,
            Err(e) => {
                eprintln!("❌ Ошибка компиляции OpenCL kernel: {}", e);
                return Err(e.into());
            }
        };

    println!("✅ OpenCL: {}", pro_que.device().name()?);
    println!("   Max work group size: {}", pro_que.device().max_wg_size()?);

    println!("\n💾 БД в RAM (binary search lookup)");
    let stats = db.stats();
    println!("   Записей: {}", stats.filled_records);
    println!("   Размер: {} MB\n", stats.size_mb);

    let batch_size = BATCH_SIZE;
    
    let result_addresses: Buffer<u8> = pro_que.buffer_builder()
        .len(batch_size * 71)  // 3 BTC addresses: 25+25+21 bytes
        .flags(flags::MEM_WRITE_ONLY)
        .build()?;

    let result_mnemonics: Buffer<u8> = pro_que.buffer_builder()
        .len(batch_size * 192)
        .flags(flags::MEM_WRITE_ONLY)
        .build()?;

    println!("✅ GPU Worker готов! (batch_size={})\n", batch_size);

    loop {
        println!("📥 Запрос работы...");
        let work = match get_work() {
            Ok(w) => w,
            Err(e) => {
                eprintln!("❌ Ошибка: {}", e);
                std::thread::sleep(std::time::Duration::from_secs(5));
                continue;
            }
        };

        let mut processed = 0u64;
        while processed < work.batch_size {
            let chunk_size = std::cmp::min(batch_size as u64, work.batch_size - processed);
            let chunk_offset = work.start_offset + processed;

            println!("🔥 GPU: offset={}, size={}", chunk_offset, chunk_size);

            let local_work_size = 64;
            let global_work_size = ((chunk_size as usize + local_work_size - 1) / local_work_size) * local_work_size;

            let kernel = pro_que.kernel_builder("generate_btc_addresses")
                .arg(&result_addresses)
                .arg(&result_mnemonics)
                .arg(chunk_offset)
                .arg(chunk_size as u32)
                .global_work_size(global_work_size)
                .local_work_size(local_work_size)
                .build()?;

            unsafe { kernel.enq()?; }
            pro_que.queue().finish()?;

            let mut addresses_bytes = vec![0u8; chunk_size as usize * 71];
            result_addresses.read(&mut addresses_bytes).enq()?;

            let mut mnemonics_data = vec![0u8; chunk_size as usize * 192];
            result_mnemonics.read(&mut mnemonics_data).enq()?;

            // CPU lookup с Base58/Bech32 декодированием
            use bech32::{ToBase32, Variant};

            print!("   🔍 CPU lookup (Base58/Bech32)...");
            let mut found_count = 0;

            for i in 0..chunk_size as usize {
                // Читаем 71 байт: P2PKH (25) + P2SH (25) + P2WPKH (21)
                let addr_bytes = &addresses_bytes[i * 71..(i + 1) * 71];

                let p2pkh_bytes = &addr_bytes[0..25];
                let p2sh_bytes = &addr_bytes[25..50];
                let p2wpkh_bytes = &addr_bytes[50..71];

                // Проверка в БД (hash160 - байты 1-20 из каждого типа адреса)
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

                    // Декодируем адреса в Base58/Bech32
                    let p2pkh_addr = bs58::encode(p2pkh_bytes).into_string();
                    let p2sh_addr = bs58::encode(p2sh_bytes).into_string();
                    let p2wpkh_hash = &p2wpkh_bytes[1..21];
                    let p2wpkh_addr = match bech32::encode("bc", p2wpkh_hash.to_vec().to_base32(), Variant::Bech32) {
                        Ok(addr) => addr,
                        Err(_) => "error".to_string(),
                    };

                    println!("\n\n🎉🎉🎉 НАЙДЕНО! 🎉🎉🎉");
                    println!("Мнемоника: {}", mnemonic_clean);
                    println!("P2PKH (Legacy):       {}", p2pkh_addr);
                    println!("P2SH (SegWit):        {}", p2sh_addr);
                    println!("Bech32 (Native SegWit): {}", p2wpkh_addr);

                    let addrs = vec![p2pkh_addr.clone(), p2sh_addr.clone(), p2wpkh_addr.clone()];
                    if let Err(e) = log_solution(work.offset_for_server + i as u128, mnemonic_clean.to_string(), addrs) {
                        eprintln!("⚠️ Ошибка: {}", e);
                    }
                    found_count += 1;
                }
            }
            if found_count > 0 {
                println!(" done (найдено: {})", found_count);
            } else {
                println!(" done");
            }

            processed += chunk_size;
            println!("   ✓ {}/{}", processed, work.batch_size);
        }

        println!("✅ Batch завершён\n");
        if let Err(e) = log_work_complete(work.offset_for_server) {
            eprintln!("⚠️ Ошибка: {}", e);
        }
    }
}

// === Main ===

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Set UTF-8 console on Windows
    #[cfg(windows)]
    {
        use std::process::Command;
        let _ = Command::new("cmd").args(["/C", "chcp 65001"]).output();
    }

    println!("=== Bitcoin BIP39 Recovery - GPU Worker ===\n");

    println!("Задача: 24-словная BIP39 мнемоника для Bitcoin");
    println!("  Известно: 20 слов");
    println!("  Неизвестно: 4 слова на позициях 20, 21, 22, 23");
    println!("  Checksum оптимизация: 2048³ x 8 = 68,719,476,736 комбинаций\n");

    println!("Структура мнемоники:");
    println!("  0-4:   switch real throw crush baby");
    println!("  5-9:   oven jazz steak donate carbon");
    println!("  10-14: fever vague yellow three control");
    println!("  15-19: flavor sugar salad base false");
    println!("  20:    ??? (unknown)");
    println!("  21:    ??? (unknown)");
    println!("  22:    ??? (unknown)");
    println!("  23:    ??? (checksum)\n");

    println!("📦 Загрузка базы данных в RAM...");
    let db = Database::load(DATABASE_PATH)?;
    let stats = db.stats();

    println!("✅ База данных загружена:");
    println!("   Всего записей: {}", stats.total_records);
    println!("   Заполненных: {} ({:.1}%)", stats.filled_records, stats.load_factor * 100.0);
    println!("   Размер: {} MB", stats.size_mb);

    println!("\n🔗 Проверка оркестратора...");
    println!("   URL: {}", WORK_SERVER_URL);

    match reqwest::blocking::get(&format!("{}/status", WORK_SERVER_URL)) {
        Ok(_) => println!("✅ Оркестратор доступен"),
        Err(_) => {
            println!("⚠️ Оркестратор недоступен!");
            return Err("Orchestrator not available".into());
        }
    }

    run_gpu_worker(&db)?;

    Ok(())
}
