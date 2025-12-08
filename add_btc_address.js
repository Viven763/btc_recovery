// Добавление Bitcoin адреса в базу данных (seedrecover hash table format)
const fs = require('fs');
const crypto = require('crypto');

const DB_PATH = 'btc_database.db';
const ADDRESS_TO_ADD = '1J8nHk7cRaHGDJmXoG2WwnARpDAMi5NCbE';

// Base58 алфавит
const BASE58_ALPHABET = '123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz';

function base58Decode(str) {
    // Считаем ведущие '1' (это нули в base58)
    let leadingZeros = 0;
    for (let i = 0; i < str.length && str[i] === '1'; i++) {
        leadingZeros++;
    }
    
    // Конвертируем из base58 в bigint
    let num = BigInt(0);
    for (let i = 0; i < str.length; i++) {
        const char = str[i];
        const index = BASE58_ALPHABET.indexOf(char);
        if (index === -1) {
            throw new Error(`Invalid Base58 character: ${char}`);
        }
        num = num * BigInt(58) + BigInt(index);
    }
    
    // Конвертируем bigint в байты
    const bytes = [];
    while (num > 0n) {
        bytes.unshift(Number(num % 256n));
        num = num / 256n;
    }
    
    // Добавляем ведущие нули
    const result = Buffer.alloc(leadingZeros + bytes.length);
    for (let i = 0; i < leadingZeros; i++) {
        result[i] = 0;
    }
    for (let i = 0; i < bytes.length; i++) {
        result[leadingZeros + i] = bytes[i];
    }
    
    return result;
}

function decodeBitcoinAddress(address) {
    // Декодируем Base58
    const decoded = base58Decode(address);
    
    // Проверяем длину (должна быть 25 байт: version + hash160 + checksum)
    if (decoded.length !== 25) {
        throw new Error(`Invalid address length: ${decoded.length}, expected 25`);
    }
    
    const version = decoded[0];
    const hash160 = decoded.slice(1, 21);
    const checksum = decoded.slice(21, 25);
    
    // Проверяем checksum
    const payload = decoded.slice(0, 21);
    const hash = crypto.createHash('sha256').update(payload).digest();
    const hash2 = crypto.createHash('sha256').update(hash).digest();
    const expectedChecksum = hash2.slice(0, 4);
    
    if (!checksum.equals(expectedChecksum)) {
        throw new Error(`Invalid checksum! Got: ${checksum.toString('hex')}, expected: ${expectedChecksum.toString('hex')}`);
    }
    
    console.log('✅ Address decoded successfully:');
    console.log('   Version:', version === 0 ? '0x00 (P2PKH mainnet)' : `0x${version.toString(16)}`);
    console.log('   Hash160:', hash160.toString('hex'));
    console.log('   Checksum:', checksum.toString('hex'), '✓');
    
    return hash160;
}

function readDbMetadata() {
    const fd = fs.openSync(DB_PATH, 'r');
    const headerBuf = Buffer.alloc(4096);
    fs.readSync(fd, headerBuf, 0, 4096, 0);
    fs.closeSync(fd);
    
    const headerStr = headerBuf.toString('utf8');
    
    const dbLengthMatch = headerStr.match(/'_dbLength':\s*(\d+)/);
    const hashMaskMatch = headerStr.match(/'_hash_mask':\s*(\d+)/);
    const lenMatch = headerStr.match(/'_len':\s*(\d+)/);
    
    return {
        dbLength: parseInt(dbLengthMatch[1]),
        hashMask: parseInt(hashMaskMatch[1]),
        len: parseInt(lenMatch[1])
    };
}

function addAddressToDb(hash160) {
    const HEADER_SIZE = 0x10000; // 64 KB
    const RECORD_SIZE = 8;
    
    const metadata = readDbMetadata();
    console.log('\n📊 Database metadata:');
    console.log('   DB Length:', metadata.dbLength.toLocaleString(), 'slots');
    console.log('   Hash Mask:', '0x' + metadata.hashMask.toString(16));
    console.log('   Current addresses:', metadata.len.toLocaleString());
    
    // Берём первые 8 байт hash160 для записи
    const hash160_prefix = hash160.slice(0, 8);
    console.log('\n📝 Record to add:', hash160_prefix.toString('hex'));
    
    // Вычисляем индекс (первые 4 байта как uint32 LE)
    const hashValue = hash160.readUInt32LE(0);
    let index = hashValue & metadata.hashMask;
    
    console.log('   Hash value:', '0x' + hashValue.toString(16));
    console.log('   Initial index:', index);
    
    const fd = fs.openSync(DB_PATH, 'r+');
    
    // Linear probing для поиска свободного слота
    const maxProbes = 10000;
    let probeCount = 0;
    const recordBuf = Buffer.alloc(RECORD_SIZE);
    
    while (probeCount < maxProbes) {
        const offset = HEADER_SIZE + (index * RECORD_SIZE);
        fs.readSync(fd, recordBuf, 0, RECORD_SIZE, offset);
        
        // Если слот пустой - записываем
        if (recordBuf.every(b => b === 0)) {
            console.log(`\n✅ Found empty slot at index ${index} (offset: 0x${offset.toString(16)})`);
            fs.writeSync(fd, hash160_prefix, 0, RECORD_SIZE, offset);
            
            // Проверяем что записалось
            fs.readSync(fd, recordBuf, 0, RECORD_SIZE, offset);
            console.log('   Written:', recordBuf.toString('hex'));
            console.log('   Probes needed:', probeCount);
            
            fs.closeSync(fd);
            return { success: true, index, offset, probes: probeCount };
        }
        
        // Если адрес уже есть
        if (recordBuf.equals(hash160_prefix)) {
            console.log(`\nℹ️ Address already exists at index ${index}`);
            fs.closeSync(fd);
            return { success: true, index, offset: HEADER_SIZE + (index * RECORD_SIZE), alreadyExists: true };
        }
        
        // Коллизия - следующий слот
        index = (index + 1) % metadata.dbLength;
        probeCount++;
    }
    
    console.log(`\n❌ Failed to find empty slot after ${maxProbes} probes`);
    fs.closeSync(fd);
    return { success: false };
}

function verifyAddress(hash160) {
    const HEADER_SIZE = 0x10000;
    const RECORD_SIZE = 8;
    
    const metadata = readDbMetadata();
    const hash160_prefix = hash160.slice(0, 8);
    const hashValue = hash160.readUInt32LE(0);
    let index = hashValue & metadata.hashMask;
    
    const fd = fs.openSync(DB_PATH, 'r');
    const recordBuf = Buffer.alloc(RECORD_SIZE);
    const maxProbes = 10000;
    
    for (let probe = 0; probe < maxProbes; probe++) {
        const offset = HEADER_SIZE + (index * RECORD_SIZE);
        fs.readSync(fd, recordBuf, 0, RECORD_SIZE, offset);
        
        if (recordBuf.every(b => b === 0)) {
            fs.closeSync(fd);
            return false;
        }
        
        if (recordBuf.equals(hash160_prefix)) {
            fs.closeSync(fd);
            return true;
        }
        
        index = (index + 1) % metadata.dbLength;
    }
    
    fs.closeSync(fd);
    return false;
}

// Main
console.log('=== Adding Bitcoin Address to Database ===\n');
console.log('Address:', ADDRESS_TO_ADD);

try {
    // 1. Декодируем адрес
    const hash160 = decodeBitcoinAddress(ADDRESS_TO_ADD);
    
    // 2. Добавляем в БД
    const result = addAddressToDb(hash160);
    
    if (result.success) {
        // 3. Верифицируем
        console.log('\n🔍 Verifying...');
        const found = verifyAddress(hash160);
        
        if (found) {
            console.log('✅ Address verified in database!');
            console.log('\n🎉 Done! Address successfully added:');
            console.log('   BTC Address:', ADDRESS_TO_ADD);
            console.log('   Hash160:', hash160.toString('hex'));
            console.log('   Stored as:', hash160.slice(0, 8).toString('hex'));
        } else {
            console.log('❌ Verification failed!');
        }
    }
} catch (e) {
    console.error('❌ Error:', e.message);
}

