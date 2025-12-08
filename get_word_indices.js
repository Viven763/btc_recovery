const fs = require('fs');

// Читаем BIP39 словарь
const words = fs.readFileSync('./cl/english.txt', 'utf8').trim().split('\n').map(w => w.trim());

// Наша сид-фраза (20 известных слов, последние 4 неизвестны)
const phrase = 'switch over fever flavor real jazz vague sugar throw steak yellow salad crush donate three base baby carbon control false'.split(' ');

console.log('=== Word indices for BIP39 seed phrase ===\n');
console.log('Known words:', phrase.length);
console.log('Missing: word 23 and word 24 (positions 22, 23)\n');

phrase.forEach((word, i) => {
    const index = words.indexOf(word);
    console.log(`word_indices[${i}] = ${index};   // ${word}`);
});

console.log('\n// Word indices array for OpenCL kernel:');
console.log('uint word_indices[24];');
phrase.forEach((word, i) => {
    const index = words.indexOf(word);
    console.log(`word_indices[${i}] = ${index};   // ${word}`);
});
console.log('word_indices[22] = w22_idx;  // UNKNOWN');
console.log('word_indices[23] = w23_idx;  // CHECKSUM (calculated)');

