#!/usr/bin/env python3
"""
Добавление тестового адреса в БД btc-20200101-to-20250201.db
"""

import hashlib
import base58
import struct
import json

def decode_base58_address(address):
    """Декодирует Bitcoin адрес (Base58Check) в hash160"""
    try:
        decoded = base58.b58decode(address)
        # Формат: [version (1 byte)][hash160 (20 bytes)][checksum (4 bytes)]
        if len(decoded) != 25:
            raise ValueError(f"Invalid address length: {len(decoded)}")

        version = decoded[0]
        hash160 = decoded[1:21]
        checksum = decoded[21:25]

        # Проверяем checksum
        hash_check = hashlib.sha256(hashlib.sha256(decoded[:21]).digest()).digest()[:4]
        if hash_check != checksum:
            raise ValueError("Invalid checksum")

        return hash160
    except Exception as e:
        print(f"Error decoding address: {e}")
        return None

def read_db_metadata(db_path):
    """Читает метаданные из БД"""
    with open(db_path, 'rb') as f:
        header = f.read(0x10000)  # 64KB header

    # Ищем JSON метаданные
    header_str = header.decode('utf-8', errors='ignore')
    json_start = header_str.find('{')
    json_end = header_str.find('}', json_start) + 1

    if json_start == -1 or json_end == 0:
        raise ValueError("Metadata not found")

    json_str = header_str[json_start:json_end]

    # Парсим метаданные
    metadata = {}
    for line in json_str.split(','):
        if "'_dbLength':" in line:
            metadata['db_length'] = int(line.split(':')[1].strip())
        elif "'_hash_mask':" in line:
            metadata['hash_mask'] = int(line.split(':')[1].strip())
        elif "'_len':" in line:
            metadata['len'] = int(line.split(':')[1].strip())

    return metadata

def add_address_to_db(db_path, hash160):
    """Добавляет адрес в БД (hash table with linear probing)"""
    metadata = read_db_metadata(db_path)

    db_length = metadata['db_length']
    hash_mask = metadata['hash_mask']

    # Вычисляем индекс
    hash_value = struct.unpack('<I', hash160[:4])[0]
    index = (hash_value & hash_mask) % db_length

    print(f"Hash160: {hash160.hex()}")
    print(f"Hash value: 0x{hash_value:08x}")
    print(f"Initial index: {index}")

    # Открываем БД для записи
    with open(db_path, 'r+b') as f:
        # Ищем пустой слот (linear probing)
        probe_count = 0
        while probe_count < 1000:
            offset = 0x10000 + (index * 8)
            f.seek(offset)
            record = f.read(8)

            # Если слот пустой - записываем
            if record == b'\x00' * 8:
                f.seek(offset)
                f.write(hash160[:8])
                print(f"✅ Адрес добавлен в слот {index} (offset: 0x{offset:x})")
                return True

            # Если адрес уже есть
            if record == hash160[:8]:
                print(f"ℹ️ Адрес уже есть в БД (слот {index})")
                return True

            # Коллизия - пробуем следующий слот
            index = (index + 1) % db_length
            probe_count += 1

        print(f"❌ Не удалось найти свободный слот (проверено {probe_count} слотов)")
        return False

def main():
    address = "1J8nHk7cRaHGDJmXoG2WwnARpDAMi5NCbE"
    db_path = "btc-20200101-to-20250201.db"

    print(f"🔍 Декодирование адреса: {address}")
    hash160 = decode_base58_address(address)

    if hash160 is None:
        print("❌ Не удалось декодировать адрес")
        return

    print(f"\n📝 Добавление в БД: {db_path}")
    add_address_to_db(db_path, hash160)

    print(f"\n✅ Готово! Теперь можно искать по seed фразе.")

if __name__ == "__main__":
    main()
