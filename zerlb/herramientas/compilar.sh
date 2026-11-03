#!/bin/sh
# Compila la CLI y el ayudante de macOS en modo release y, si se pide, los instala.
# Uso: sh zerlb/herramientas/compilar.sh [--instalar CARPETA]
# Al instalar, guarda los binarios que había en CARPETA/anterior/ para poder volver atrás.
# Respeta CARGO_TARGET_DIR: la compilación pesa unos 2 GB, conviene dejarla fuera de carpetas
# que se respaldan o sincronizan (por ejemplo CARGO_TARGET_DIR=~/.cargo-target/agent-desktop).

set -eu
cd "$(git rev-parse --show-toplevel)"
salida="${CARGO_TARGET_DIR:-target}/release"
cargo build --locked --release -p agent-desktop --bin agent-desktop
cargo build --locked --release -p agent-desktop-macos --bin agent-desktop-macos-helper
echo "Compilado: $("$salida/agent-desktop" version)"
echo "Commit: $(git describe --tags --always --dirty)"

if [ "${1:-}" = "--instalar" ]; then
    destino="${2:?Falta la carpeta de destino}"
    mkdir -p "$destino/anterior"
    for archivo in agent-desktop agent-desktop-macos-helper; do
        [ -f "$destino/$archivo" ] && cp -p "$destino/$archivo" "$destino/anterior/$archivo"
        install -m 755 "$salida/$archivo" "$destino/$archivo"
    done
    git describe --tags --always --dirty > "$destino/COMPILACION"
    echo "Instalado en $destino (lo anterior quedó en $destino/anterior/)."
fi
