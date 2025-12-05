# novaria-language

ЯП с компиляцией под ELF, PE, NVM байткод.

Быстрый старт:
```bash
git clone https://github.com/noxzion/novaria-language.git
cd novaria-language
cargo build --release
```

## Компиляция программ

### Windows (PE):
```bash
.\target\release\novaira-language.exe examples\main.nl
.\examples\main.exe
```

### Linux (ELF):
```bash
./target/release/novaira-language examples/main.nl --elf
./examples/main
```

### NVM байткод (для NovariaOS):
```bash
# Компиляция в BIN (NovariaOS executable)
.\t\target\release\novaira-language.exe examples\test_nvm.nl --novaria
# Создаст файл examples\test_nvm.bin для запуска в NovariaOS

# Генерация читаемого ассемблерного кода
.\target\release\novaira-language.exe examples\test_nvm.nl --nvm-code
# Создаст файл examples\test_nvm.asm с NVM assembly кодом
```