
#!/usr/bin/env python3
"""
Автоматическая конвертация Ethereum recovery в Bitcoin recovery
Применяет все необходимые изменения в src/main.rs
"""

def convert_main_rs():
    """Конвертирует main.rs из Ethereum в Bitcoin версию"""

    with open("src/main.rs", "r") as f:
        content = f.read()

    # 1. Заголовок
    content = content.replace(
        "// Ethereum BIP39 Recovery Tool",
        "// Bitcoin BIP39 Recovery Tool"
    )
    content = content.replace(
        "// GPU генерирует адреса, CPU проверяет в БД",
        "// GPU генерирует 3 типа BTC адресов (P2PKH/P2SH/P2WPKH), CPU проверяет в БД"
    )

    # 2. DATABASE_PATH
    content = content.replace(
        'const DATABASE_PATH: &str = "eth20240925";',
        'const DATABASE_PATH: &str = "btc_addresses_db";'
    )

    # 3. Убрать Keccak256 и eth_address.cl
    content = content.replace(
        '        "keccak256.cl",',
        '        // "keccak256.cl",  // Not needed for Bitcoin'
    )
    content = content.replace(
        '        "eth_address.cl",',
        '        "btc_address.cl",  // Bitcoin address derivation'
    )

    # 4. Заменить kernel name
    content = content.replace(
        '__kernel void generate_eth_addresses(',
        '__kernel void generate_btc_addresses('
    )

    # 5. Обновить комментарии в kernel
    content = content.replace(
        '// === ОПТИМИЗИРОВАННЫЙ GPU Address Generator Kernel ===',
        '// === Bitcoin Address Generator Kernel ==='
    )
    content = content.replace(
        '// 22 известных слова + 2 неизвестных = 24 слова',
        '// Генерирует 3 типа Bitcoin адресов: P2PKH (1...), P2SH (3...), P2WPKH (bc1...)'
    )

    # 6. Заменить вывод адресов на BTC
    content = content.replace(
        '__global ulong *result_addresses,     // Output: массив addr_suffix (8 bytes каждый)',
        '__global uchar *result_addresses,     // Output: 71 байт на комбинацию (P2PKH 25 + P2SH 25 + P2WPKH 21)'
    )

    # 7. Заменить ETH адрес на BTC адреса
    old_eth_derive = '''    // Derive Ethereum address at index 0 (m/44'/60'/0'/0/0)
    // To check multiple addresses, run this worker multiple times with different indices
    uchar eth_address[20];
    for(int i = 0; i < 20; i++) eth_address[i] = 0;
    derive_eth_address_bip44(seed, eth_address);

    // Extract addr_suffix (last 8 bytes)
    ulong addr_suffix = 0;
    for(int i = 0; i < 8; i++) {
        addr_suffix |= ((ulong)eth_address[12 + i]) << (i * 8);
    }

    // Write results
    result_addresses[gid] = addr_suffix;'''

    new_btc_derive = '''    // Derive all 3 Bitcoin address types
    // P2PKH (m/44'/0'/0'/0/0), P2SH (m/49'/0'/0'/0/0), P2WPKH (m/84'/0'/0'/0/0)
    uchar all_btc_addresses[71];  // 25 + 25 + 21 bytes
    for(int i = 0; i < 71; i++) all_btc_addresses[i] = 0;

    derive_all_btc_addresses(seed, all_btc_addresses);

    // Write results (71 bytes per address set)
    for(int i = 0; i < 71; i++) {
        result_addresses[gid * 71 + i] = all_btc_addresses[i];
    }'''

    content = content.replace(old_eth_derive, new_btc_derive)

    # 8. Изменить buffer creation
    content = content.replace(
        'let result_addresses: Buffer<u64> = pro_que.buffer_builder()\n        .len(batch_size)',
        'let result_addresses: Buffer<u8> = pro_que.buffer_builder()\n        .len(batch_size * 71)  // 3 BTC addresses: 25+25+21 bytes'
    )

    # 9. Изменить kernel name в вызове
    content = content.replace(
        '.kernel_builder("generate_eth_addresses")',
        '.kernel_builder("generate_btc_addresses")'
    )

    # 10. Изменить чтение результатов
    content = content.replace(
        'let mut addresses = vec![0u64; chunk_size as usize];',
        'let mut addresses_bytes = vec![0u8; chunk_size as usize * 71];'
    )
    content = content.replace(
        'result_addresses.read(&mut addresses).enq()?;',
        'result_addresses.read(&mut addresses_bytes).enq()?;'
    )

    # 11. Обновить main function
    content = content.replace(
        'println!("=== Ethereum BIP39 Recovery - GPU Worker ===\\n");',
        'println!("=== Bitcoin BIP39 Recovery - GPU Worker ===\\n");'
    )
    content = content.replace(
        'println!("Задача: 24-словная BIP39 мнемоника для Ethereum");',
        'println!("Задача: 24-словная BIP39 мнемоника для Bitcoin");'
    )

    # Сохраняем
    with open("src/main.rs", "w") as f:
        f.write(content)

    print("✅ main.rs успешно конвертирован для Bitcoin!")
    print("\n⚠️  ВНИМАНИЕ: Вам всё ещё нужно:")
    print("   1. Вручную заменить CPU lookup код (строки ~400-430)")
    print("   2. Добавить Base58 и Bech32 декодирование")
    print("   3. Обновить db_loader.rs для Bitcoin формата")
    print("\nПодробности см. в BITCOIN_CHANGES.md")

if __name__ == "__main__":
    import os
    if not os.path.exists("src/main.rs"):
        print("❌ Ошибка: Запустите из корня проекта btc_recovery/")
        exit(1)

    # Создаём backup
    import shutil
    shutil.copy("src/main.rs", "src/main.rs.eth_backup")
    print("📦 Создан backup: src/main.rs.eth_backup")

    convert_main_rs()
