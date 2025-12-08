// Проверка формата записей в Bitcoin базе данных (seedrecover hash table)
const fs = require('fs');

const DB_PATH = 'btc_database.db';

// Открываем файл
const fd = fs.openSync(DB_PATH, 'r');
const stats = fs.statSync(DB_PATH);

console.log('=== BITCOIN DATABASE INFO ===');
console.log('File:', DB_PATH);
console.log('Database size:', (stats.size / 1024 / 1024 / 1024).toFixed(2), 'GB');
console.log('Database size (bytes):', stats.size);

// Читаем заголовок (64 KB = 0x10000 байт)
const HEADER_SIZE = 0x10000;
const headerBuf = Buffer.alloc(4096); // Читаем первые 4KB для метаданных
fs.readSync(fd, headerBuf, 0, 4096, 0);

console.log('\n=== HEADER (first 1000 bytes as string) ===');
// Находим конец значимых данных (до нулей)
let headerEnd = 0;
for (let i = 0; i < headerBuf.length; i++) {
    if (headerBuf[i] === 0x00) {
        headerEnd = i;
        break;
    }
}
console.log(headerBuf.slice(0, Math.min(headerEnd, 1000)).toString());

// Парсим метаданные
const headerStr = headerBuf.toString('utf8', 0, headerEnd);
console.log('\n=== PARSED METADATA ===');

// Ищем JSON часть
const jsonMatch = headerStr.match(/\{[^}]+\}/);
if (jsonMatch) {
    console.log('JSON metadata found:', jsonMatch[0]);
    
    // Парсим значения вручную
    const dbLengthMatch = headerStr.match(/'_dbLength':\s*(\d+)/);
    const hashMaskMatch = headerStr.match(/'_hash_mask':\s*(\d+)/);
    const lenMatch = headerStr.match(/'_len':\s*(\d+)/);
    const bytesPerAddrMatch = headerStr.match(/'_bytes_per_addr':\s*(\d+)/);
    
    const dbLength = dbLengthMatch ? parseInt(dbLengthMatch[1]) : 0;
    const hashMask = hashMaskMatch ? parseInt(hashMaskMatch[1]) : 0;
    const len = lenMatch ? parseInt(lenMatch[1]) : 0;
    const bytesPerAddr = bytesPerAddrMatch ? parseInt(bytesPerAddrMatch[1]) : 8;
    
    console.log('  DB Length (slots):', dbLength.toLocaleString());
    console.log('  Hash Mask:', '0x' + hashMask.toString(16));
    console.log('  Addresses count:', len.toLocaleString());
    console.log('  Bytes per address:', bytesPerAddr);
    console.log('  Load factor:', ((len / dbLength) * 100).toFixed(2) + '%');
}

// Читаем первые 10 непустых записей после заголовка
console.log('\n=== FIRST 10 NON-EMPTY RECORDS ===');
const recordBuf = Buffer.alloc(8);
let foundCount = 0;
let offset = HEADER_SIZE;
const maxCheck = 100000; // Проверяем до 100K записей

for (let i = 0; i < maxCheck && foundCount < 10; i++) {
    fs.readSync(fd, recordBuf, 0, 8, offset + i * 8);
    
    // Проверяем что запись не пустая
    const isEmpty = recordBuf.every(b => b === 0);
    if (!isEmpty) {
        console.log(`Slot ${i}: ${recordBuf.toString('hex')} (offset: 0x${(offset + i * 8).toString(16)})`);
        foundCount++;
    }
}

// Читаем последние 10 непустых записей
console.log('\n=== CHECKING LAST PORTION OF DB ===');
const dbLength = 1073741824; // 2^30 slots (стандартный размер)
const endOffset = HEADER_SIZE + (dbLength - 1000) * 8;

foundCount = 0;
for (let i = 0; i < 1000 && foundCount < 10; i++) {
    const pos = endOffset + i * 8;
    if (pos >= stats.size) break;
    
    fs.readSync(fd, recordBuf, 0, 8, pos);
    const isEmpty = recordBuf.every(b => b === 0);
    if (!isEmpty) {
        const slotNum = (pos - HEADER_SIZE) / 8;
        console.log(`Slot ${slotNum}: ${recordBuf.toString('hex')} (offset: 0x${pos.toString(16)})`);
        foundCount++;
    }
}

fs.closeSync(fd);
console.log('\n✅ Database check complete!');

