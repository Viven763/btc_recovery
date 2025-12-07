// Bitcoin Database Loader
// Формат: seedrecover hash table
// - Hash table с открытой адресацией
// - 8 байт на запись (первые 8 байт hash160)
// - Заголовок: 64 KB (0x10000 байт)

use std::fs::File;
use std::io::{Read, Seek, SeekFrom};

const HEADER_SIZE: u64 = 0x10000;  // 64 KB
const RECORD_SIZE: usize = 8;       // 8 bytes per record

pub struct Database {
    file: File,
    db_length: u64,        // Количество слотов в hash table
    hash_mask: u32,        // Маска для hash функции
    len: u64,              // Количество реальных адресов
    table_offset: u64,     // Offset начала таблицы (после заголовка)
}

pub struct DatabaseStats {
    pub total_records: usize,
    pub filled_records: usize,
    pub size_mb: usize,
    pub load_factor: f64,
}

impl Database {
    pub fn load(path: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let mut file = File::open(path)?;

        // Читаем заголовок
        let mut header = vec![0u8; HEADER_SIZE as usize];
        file.read_exact(&mut header)?;

        // Парсим метаданные (JSON после "seedrecover address database\r\n")
        let metadata = Self::parse_metadata(&header)?;

        println!("📊 БД статистика:");
        println!("   Слотов в таблице: {}", metadata.db_length);
        println!("   Реальных адресов: {}", metadata.len);
        println!("   Load factor: {:.1}%", (metadata.len as f64 / metadata.db_length as f64) * 100.0);

        Ok(Database {
            file,
            db_length: metadata.db_length,
            hash_mask: metadata.hash_mask,
            len: metadata.len,
            table_offset: HEADER_SIZE,
        })
    }

    fn parse_metadata(header: &[u8]) -> Result<Metadata, Box<dyn std::error::Error>> {
        // Ищем JSON метаданные в заголовке
        let header_str = String::from_utf8_lossy(header);

        // Находим начало и конец JSON
        let json_start = header_str.find('{').ok_or("Metadata not found")?;
        let json_end = header_str[json_start..].find('}').ok_or("Metadata end not found")? + json_start + 1;

        let json_str = &header_str[json_start..json_end];

        // Парсим вручную (простой парсер для этого формата)
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

    /// Поиск Bitcoin адреса по hash160
    /// hash160 = RIPEMD160(SHA256(pubkey)) - 20 байт
    /// БД хранит первые 8 байт из hash160
    pub fn lookup_bitcoin_address(&mut self, hash160: &[u8]) -> bool {
        if hash160.len() != 20 {
            return false;
        }

        // Берём первые 8 байт
        let hash_prefix: [u8; 8] = hash160[0..8].try_into().unwrap();

        // Вычисляем hash для индекса в таблице
        // Используем первые 4 байта как uint32 для hash функции
        let hash_value = u32::from_le_bytes([hash160[0], hash160[1], hash160[2], hash160[3]]);
        let mut index = (hash_value & self.hash_mask) as u64;

        // Open addressing: линейный пробинг
        let max_probes = 100;  // Максимум проверок (чтобы не зависнуть)

        for _ in 0..max_probes {
            // Читаем запись по индексу
            let offset = self.table_offset + (index * RECORD_SIZE as u64);

            if self.file.seek(SeekFrom::Start(offset)).is_err() {
                return false;
            }

            let mut record = [0u8; RECORD_SIZE];
            if self.file.read_exact(&mut record).is_err() {
                return false;
            }

            // Проверка:
            // 1. Если запись = 0x00..00 → пустой слот, адрес не найден
            // 2. Если запись совпадает с hash_prefix → найдено!
            // 3. Иначе → коллизия, пробуем следующий слот

            if record == [0u8; 8] {
                // Пустой слот - адрес не найден
                return false;
            }

            if record == hash_prefix {
                // Найдено!
                return true;
            }

            // Коллизия - пробуем следующий слот (linear probing)
            index = (index + 1) % self.db_length;
        }

        // Достигнут лимит проверок
        false
    }

    pub fn stats(&self) -> DatabaseStats {
        DatabaseStats {
            total_records: self.db_length as usize,
            filled_records: self.len as usize,
            size_mb: ((self.db_length * RECORD_SIZE as u64) / (1024 * 1024)) as usize,
            load_factor: self.len as f64 / self.db_length as f64,
        }
    }
}

struct Metadata {
    db_length: u64,
    hash_mask: u32,
    len: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_db_load() {
        // Тест загрузки БД
        let db = Database::load("btc-20200101-to-20250201.db");
        assert!(db.is_ok());
    }

    #[test]
    fn test_lookup() {
        let mut db = Database::load("btc-20200101-to-20250201.db").unwrap();

        // Тест поиска (нужен реальный hash160 из БД)
        let test_hash160 = [0u8; 20];  // Placeholder
        let found = db.lookup_bitcoin_address(&test_hash160);

        // Этот тест пройдёт только если hash160 реально есть в БД
        println!("Lookup test: found = {}", found);
    }
}
