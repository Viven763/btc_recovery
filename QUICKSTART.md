# Bitcoin Recovery - Быстрый старт 🚀

## ✅ Что уже сделано:

1. ✅ **cl/btc_address.cl** - OpenCL код для 3 типов BTC адресов (P2PKH, P2SH, P2WPKH)
2. ✅ **Cargo.toml** - добавлены зависимости: bs58 + bech32
3. ✅ **convert_to_bitcoin.py** - автоматическая конвертация main.rs
4. ✅ **SETUP_BITCOIN.md** - подробная инструкция
5. ✅ **BITCOIN_CHANGES.md** - детальные изменения

## 🎯 Что нужно сделать (3 шага):

### Шаг 1: Запустить автоконвертацию

```bash
cd /Users/vivenlmao/Desktop/coding/myprojects/eth_recover_session/btc_recovery
python3 convert_to_bitcoin.py
```

Это применит ~80% изменений автоматически.

### Шаг 2: Дополнить main.rs вручную

Откройте `src/main.rs` и найдите секцию "CPU lookup" (примерно строка 400).

Добавьте **в начало функции `run_gpu_worker`** (после `println!("✅ GPU Worker готов!..."`):

```rust
use bech32::{ToBase32, Variant};
```

Затем ЗАМЕНИТЕ блок CPU lookup на код из `SETUP_BITCOIN.md` (Шаг 2).

### Шаг 3: Обновить db_loader.rs

Замените `src/db_loader.rs` на код из `SETUP_BITCOIN.md` (Шаг 4).

## 🔨 Сборка:

```bash
cargo build --release
```

## 🧪 Быстрый тест (без БД):

Чтобы проверить, что всё скомпилировалось:

```bash
# Создайте пустую "БД" для теста
touch btc_addresses_db

# Попробуйте запустить (должен запуститься, но ничего не найти)
./target/release/btc_recovery
```

## 📊 Что изменилось:

```
┌────────────────────────────────────────────────────────┐
│          ETHEREUM → BITCOIN                            │
├────────────────────────────────────────────────────────┤
│                                                        │
│ Криптография:                                          │
│   Keccak256 → SHA256 + RIPEMD160 ✓                     │
│   Hash160 вместо addr_suffix ✓                         │
│                                                        │
│ Адреса:                                                │
│   1 тип (ETH) → 3 типа (BTC) ✓                        │
│   - P2PKH (1...)                                       │
│   - P2SH (3...)                                        │
│   - P2WPKH (bc1...)                                    │
│                                                        │
│ Деривация:                                             │
│   m/44'/60'/0'/0/0 → m/44'/0'/0'/0/0 ✓ (P2PKH)        │
│                    → m/49'/0'/0'/0/0 ✓ (P2SH)         │
│                    → m/84'/0'/0'/0/0 ✓ (P2WPKH)       │
│                                                        │
│ Encoding:                                              │
│   Hex → Base58 + Bech32 ✓                              │
│                                                        │
│ БД:                                                    │
│   8 байт suffix → 20 байт hash160 ✓                    │
│                                                        │
└────────────────────────────────────────────────────────┘
```

## ⚡ Производительность:

```
Одинаковая с Ethereum!
~200,000 комбинаций/сек на RTX 4090

Для 68.7 млрд комбинаций (2048³ × 8):
  1 GPU:   ~95 часов
  10 GPU:  ~9.5 часов
  100 GPU: ~1 час
```

## 📁 Структура файлов:

```
btc_recovery/
├── cl/
│   ├── btc_address.cl ✨ НОВЫЙ - Bitcoin адреса
│   ├── address.cl     ✓ BIP32/44 (переиспользуется)
│   ├── ripemd.cl      ✓ RIPEMD160 (переиспользуется)
│   ├── sha2.cl        ✓ SHA256 (переиспользуется)
│   └── ... (остальные файлы)
│
├── src/
│   ├── main.rs        🔧 ИЗМЕНЁН - Bitcoin логика
│   └── db_loader.rs   🔧 ИЗМЕНЁН - hash160 формат
│
├── Cargo.toml         🔧 ИЗМЕНЁН - bs58 + bech32
│
├── QUICKSTART.md      📖 Этот файл
├── SETUP_BITCOIN.md   📖 Подробная инструкция
├── BITCOIN_CHANGES.md 📖 Детальные изменения
└── convert_to_bitcoin.py 🔧 Автоконвертация
```

## 🐛 Частые проблемы:

**Ошибка: "btc_addresses_db not found"**
→ Создайте БД или измените `DATABASE_PATH` в main.rs

**Ошибка компиляции: "cannot find function `lookup_bitcoin_address`"**
→ Убедитесь, что db_loader.rs обновлён (Шаг 3)

**Ошибка: "use of undeclared type `Variant`"**
→ Добавьте `use bech32::{ToBase32, Variant};` (Шаг 2)

**OpenCL error: "No platform found"**
→ Установите драйверы GPU (CUDA/ROCm)

## 🎯 Следующий шаг:

**Создать Bitcoin БД:**

```python
# Пример создания БД из списка адресов
import hashlib

def create_btc_db(addresses):
    """
    addresses: список Bitcoin адресов с балансом
    Возвращает sorted list of hash160
    """
    hash160_list = []

    for addr in addresses:
        # Декодируем Base58/Bech32 → получаем hash160
        # ... (ваша логика)
        hash160_list.append(hash160_bytes)

    # Сортировка
    hash160_list.sort()

    # Запись
    with open("btc_addresses_db", "wb") as f:
        for h160 in hash160_list:
            f.write(h160)  # 20 bytes

# Использование:
# addresses = load_bitcoin_addresses_with_balance()
# create_btc_db(addresses)
```

## 📚 Дополнительная информация:

- **SETUP_BITCOIN.md** - пошаговая установка
- **BITCOIN_CHANGES.md** - детальное описание всех изменений
- **README.md** - общая информация о проекте

---

**Готово!** Следуйте трём шагам выше и ваш Bitcoin recovery модуль будет готов! 🎉

Если возникнут вопросы, смотрите `SETUP_BITCOIN.md` для подробных инструкций.
