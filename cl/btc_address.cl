// Bitcoin Address Derivation from BIP44
// Generates 3 types: Legacy (1...), SegWit (3...), Native SegWit (bc1...)

// Bitcoin P2PKH address generation (Legacy addresses starting with '1')
// Format: version (0x00) + hash160(pubkey) + checksum (4 bytes)
void p2pkh_address_for_public_key(extended_public_key_t *pub, uchar *address_bytes) {
    // 1. Get public key hash (hash160 = RIPEMD160(SHA256(pubkey)))
    uchar pubkey_hash[20] = {0};
    identifier_for_public_key(pub, pubkey_hash);

    // 2. Add version byte (0x00 for mainnet P2PKH, addresses start with '1')
    address_bytes[0] = 0x00;

    // 3. Copy hash160 (20 bytes)
    for(int i = 0; i < 20; i++) {
        address_bytes[i + 1] = pubkey_hash[i];
    }

    // 4. Double SHA256 checksum
    uchar sha256d_result[32] = {0};
    sha256d(address_bytes, 21, sha256d_result);

    // 5. Append first 4 bytes as checksum
    address_bytes[21] = sha256d_result[0];
    address_bytes[22] = sha256d_result[1];
    address_bytes[23] = sha256d_result[2];
    address_bytes[24] = sha256d_result[3];

    // Total: 25 bytes (1 version + 20 hash160 + 4 checksum)
    // These 25 bytes are then Base58-encoded on CPU side
}

// Bitcoin P2SH-P2WPKH address generation (SegWit addresses starting with '3')
// Format: version (0x05) + hash160(redeemScript) + checksum (4 bytes)
// Already implemented in address.cl as p2shwpkh_address_for_public_key()

// Bitcoin Native SegWit (Bech32) address generation (addresses starting with 'bc1')
// Format: witness version (0x00) + hash160(pubkey) - NO checksum here (Bech32 has own checksum)
void p2wpkh_address_for_public_key(extended_public_key_t *pub, uchar *witness_program) {
    // Native SegWit witness program is just hash160 of pubkey
    // Format: version (1 byte) + hash160 (20 bytes) = 21 bytes
    // Bech32 encoding is done on CPU side

    witness_program[0] = 0x00; // Witness version 0

    // Get hash160(pubkey)
    uchar pubkey_hash[20] = {0};
    identifier_for_public_key(pub, pubkey_hash);

    // Copy hash160
    for(int i = 0; i < 20; i++) {
        witness_program[i + 1] = pubkey_hash[i];
    }

    // Total: 21 bytes (1 version + 20 hash160)
    // Bech32 encoding with checksum done on CPU
}

// Derive all 3 Bitcoin address types from BIP44/49/84 paths
// Output format: p2pkh (25 bytes) + p2sh (25 bytes) + p2wpkh (21 bytes) = 71 bytes total
void derive_all_btc_addresses(uchar *seed, uchar *all_addresses) {
    extended_private_key_t master;
    new_master_from_seed(BITCOIN_MAINNET, seed, &master);

    // === 1. P2PKH (Legacy, starts with '1') - BIP44: m/44'/0'/0'/0/0 ===
    extended_private_key_t purpose_44;
    hardened_private_child_from_private(&master, &purpose_44, 44);

    extended_private_key_t coin_44;
    hardened_private_child_from_private(&purpose_44, &coin_44, 0);

    extended_private_key_t account_44;
    hardened_private_child_from_private(&coin_44, &account_44, 0);

    extended_private_key_t change_44;
    normal_private_child_from_private(&account_44, &change_44, 0);

    extended_private_key_t address_44;
    normal_private_child_from_private(&change_44, &address_44, 0);

    extended_public_key_t pub_44;
    public_from_private(&address_44, &pub_44);

    p2pkh_address_for_public_key(&pub_44, &all_addresses[0]); // Offset 0, 25 bytes

    // === 2. P2SH-P2WPKH (SegWit, starts with '3') - BIP49: m/49'/0'/0'/0/0 ===
    extended_private_key_t purpose_49;
    hardened_private_child_from_private(&master, &purpose_49, 49);

    extended_private_key_t coin_49;
    hardened_private_child_from_private(&purpose_49, &coin_49, 0);

    extended_private_key_t account_49;
    hardened_private_child_from_private(&coin_49, &account_49, 0);

    extended_private_key_t change_49;
    normal_private_child_from_private(&account_49, &change_49, 0);

    extended_private_key_t address_49;
    normal_private_child_from_private(&change_49, &address_49, 0);

    extended_public_key_t pub_49;
    public_from_private(&address_49, &pub_49);

    p2shwpkh_address_for_public_key(&pub_49, &all_addresses[25]); // Offset 25, 25 bytes

    // === 3. P2WPKH (Native SegWit, starts with 'bc1') - BIP84: m/84'/0'/0'/0/0 ===
    extended_private_key_t purpose_84;
    hardened_private_child_from_private(&master, &purpose_84, 84);

    extended_private_key_t coin_84;
    hardened_private_child_from_private(&purpose_84, &coin_84, 0);

    extended_private_key_t account_84;
    hardened_private_child_from_private(&coin_84, &account_84, 0);

    extended_private_key_t change_84;
    normal_private_child_from_private(&account_84, &change_84, 0);

    extended_private_key_t address_84;
    normal_private_child_from_private(&change_84, &address_84, 0);

    extended_public_key_t pub_84;
    public_from_private(&address_84, &pub_84);

    p2wpkh_address_for_public_key(&pub_84, &all_addresses[50]); // Offset 50, 21 bytes
}

// Alternative: Derive multiple Bitcoin addresses (for checking multiple indices)
void derive_btc_addresses_multiple_paths(uchar *seed, uchar *addresses, int num_addresses) {
    extended_private_key_t master;
    new_master_from_seed(BITCOIN_MAINNET, seed, &master);

    extended_private_key_t purpose;
    hardened_private_child_from_private(&master, &purpose, 44);

    extended_private_key_t coin_type;
    hardened_private_child_from_private(&purpose, &coin_type, 0);

    extended_private_key_t account;
    hardened_private_child_from_private(&coin_type, &account, 0);

    extended_private_key_t change;
    normal_private_child_from_private(&account, &change, 0);

    // Derive multiple addresses (0 to num_addresses-1)
    for(int i = 0; i < num_addresses; i++) {
        extended_private_key_t address_key;
        normal_private_child_from_private(&change, &address_key, i);

        extended_public_key_t pub;
        public_from_private(&address_key, &pub);

        p2pkh_address_for_public_key(&pub, &addresses[i * 25]);
    }
}
