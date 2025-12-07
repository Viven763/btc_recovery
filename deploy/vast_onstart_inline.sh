#!/bin/bash
# Компактный on-start script для Vast.ai (можно вставить прямо в форму)
# ИЗМЕНИТЕ ПЕРЕМЕННЫЕ НИЖЕ!

ORCH="http://90.156.225.121:3000"
SECRET="15a172308d70dede515f9eecc78eaea9345b419581d0361220313d938631b12d"
DB_URL="https://cryptoguide.tips/btcrecover-addressdbs/btc-20200101-to-20250201.zip"
REPO="https://github.com/Viven763/btc_recovery.git"  # Ваш GitHub репозиторий

# === НЕ ИЗМЕНЯЙТЕ КОД НИЖЕ ===
set -e
echo "📦 Установка зависимостей..."
apt-get update -qq && apt-get install -y -qq curl wget git build-essential pkg-config libssl-dev ocl-icd-opencl-dev clinfo unzip > /dev/null 2>&1

echo "🦀 Установка Rust..."
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
source $HOME/.cargo/env

echo "📥 Клонирование репозитория..."
mkdir -p /workspace && cd /workspace
if [ -n "$REPO" ]; then
    git clone "$REPO" btc_recovery
else
    echo "❌ REPO не указан!"
    exit 1
fi

cd btc_recovery || exit 1

echo "💾 Скачивание БД (8GB)..."
if [ ! -f btc-20200101-to-20250201.db ]; then
    wget -q --show-progress "$DB_URL" -O btc-20200101-to-20250201.zip
    unzip -q btc-20200101-to-20250201.zip
    rm btc-20200101-to-20250201.zip
fi

echo "🔧 Компиляция (release mode)..."
cargo build --release

echo "✅ GPU информация:"
clinfo | grep -E "Device Name|Device Type" || true

echo "🚀 Запуск worker..."
export WORK_SERVER_URL="$ORCH"
export WORK_SERVER_SECRET="$SECRET"
export DATABASE_PATH="/workspace/btc_recovery/btc-20200101-to-20250201.db"

./target/release/btc_recovery 2>&1 | tee worker.log
