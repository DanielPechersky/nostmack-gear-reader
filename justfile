set dotenv-load
set dotenv-required

flash $ID:
    cargo run --release

monitor:
    espflash monitor