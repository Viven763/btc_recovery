// Bitcoin Database Loader - RAM version
// Загружает БД в RAM и использует binary search для быстрого lookup
// Формат: seedrecover hash table (8 байт на запись)

use std::fs::File;
use std::io::{Read, BufReader};
use std::collections::HashSet;

const HEADER_SIZE: usize = 0x10000;  // 64 KB
const RECORD_SIZE: usize = 8;         // 8 bytes per record

pub struct Database {
    // Храним только непустые записи, отсортированные для binary search
    pub records: Vec<u64>,
    pub total_slots: usize,
    pub filled_count: usize,
}

pub struct DatabaseStats {
    pub total_records: usize,
    pub filled_records: usize,
    pub size_mb: usize,
    pub load_factor: f64,
}

impl Database {
    pub fn load(path: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let file = File::open(path)?;
        let file_size = file.metadata()?.len() as usize;
        let mut reader = BufReader::with_capacity(64 * 1024 * 1024, file); // 64MB buffer

        // Читаем заголовок
        let mut header = vec![0u8; HEADER_SIZE];
        reader.read_exact(&mut header)?;

        // Парсим метаданные
        let metadata = Self::parse_metadata(&header)?;

        println!("📊 БД статистика:");
        println!("   Слотов в таблице: {}", metadata.db_length);
        println!("   Реальных адресов: {}", metadata.len);
        println!("   Load factor: {:.1}%", (metadata.len as f64 / metadata.db_length as f64) * 100.0);

        // Рассчитываем размер данных
        let data_size = file_size - HEADER_SIZE;
        let num_slots = data_size / RECORD_SIZE;

        println!("\n📦 Загрузка БД в RAM...");
        println!("   Читаем {} слотов ({} GB)...", num_slots, data_size / 1024 / 1024 / 1024);

        // Читаем все данные
        let mut data = vec![0u8; data_size];
        reader.read_exact(&mut data)?;

        println!("   Извлекаем непустые записи...");

        // Извлекаем непустые записи как u64
        let mut records: Vec<u64> = Vec::with_capacity(metadata.len as usize);
        let empty_record = [0u8; 8];

        for i in 0..num_slots {
            let offset = i * RECORD_SIZE;
            let record_bytes = &data[offset..offset + RECORD_SIZE];
            
            if record_bytes != empty_record {
                let value = u64::from_le_bytes([
                    record_bytes[0], record_bytes[1], record_bytes[2], record_bytes[3],
                    record_bytes[4], record_bytes[5], record_bytes[6], record_bytes[7],
                ]);
                records.push(value);
            }
        }

        // Освобождаем память от raw data
        drop(data);

        println!("   Найдено {} непустых записей", records.len());
        println!("   Сортировка для binary search...");

        // Сортируем для binary search
        records.sort_unstable();

        // Удаляем дубликаты (если есть)
        records.dedup();

        println!("✅ БД загружена в RAM!");
        println!("   Записей: {} ({} MB)", records.len(), records.len() * 8 / 1024 / 1024);

        Ok(Database {
            records,
            total_slots: num_slots,
            filled_count: metadata.len as usize,
        })
    }

    fn parse_metadata(header: &[u8]) -> Result<Metadata, Box<dyn std::error::Error>> {
        let header_str = String::from_utf8_lossy(header);

        let json_start = header_str.find('{').ok_or("Metadata not found")?;
        let json_end = header_str[json_start..].find('}').ok_or("Metadata end not found")? + json_start + 1;

        let json_str = &header_str[json_start..json_end];

        let mut db_length = 0u64;
        let mut hash_mask = 0u32;
        let mut len = 0u64;

        for line in json_str.split(',') {
            let line = line.trim();
            if line.contains("'_dbLength'") {
                if let Some(value) = line.split(':').nth(1) {
                    db_length = value.trim().parse()?;
                }
            } else if line.contains("'_hash_mask'") {
                if let Some(value) = line.split(':').nth(1) {
                    hash_mask = value.trim().parse()?;
                }
            } else if line.contains("'_len'") {
                if let Some(value) = line.split(':').nth(1) {
                    len = value.trim().parse()?;
                }
            }
        }

        Ok(Metadata {
            db_length,
            hash_mask,
            len,
        })
    }

    /// Быстрый lookup через binary search
    /// hash160 = 20 байт, мы храним первые 8 байт как u64
    pub fn lookup_bitcoin_address(&self, hash160: &[u8]) -> bool {
        if hash160.len() < 8 {
            return false;
        }

        // Берём первые 8 байт как u64 (little-endian)
        let search_value = u64::from_le_bytes([
            hash160[0], hash160[1], hash160[2], hash160[3],
            hash160[4], hash160[5], hash160[6], hash160[7],
        ]);

        // Binary search: O(log n) - очень быстро!
        self.records.binary_search(&search_value).is_ok()
    }

    pub fn stats(&self) -> DatabaseStats {
        DatabaseStats {
            total_records: self.total_slots,
            filled_records: self.records.len(),
            size_mb: self.records.len() * 8 / (1024 * 1024),
            load_factor: self.records.len() as f64 / self.total_slots as f64,
        }
    }
}

struct Metadata {
    db_length: u64,
    hash_mask: u32,
    len: u64,
}
