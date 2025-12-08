const fs = require('fs');

// Читаем BIP39 словарь
const words = fs.readFileSync('./cl/english.txt', 'utf8').trim().split('\n').map(w => w.trim());

// Наша сид-фраза (22 известных слова, 23 и 24 неизвестны)
const phrase = 'protect arctic pudding cabbage fiction hub extend board yard december service drip suffer fox error note mother online shield stomach engage click'.split(' ');

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

