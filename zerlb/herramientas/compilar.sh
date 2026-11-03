#!/bin/sh
# Compila la CLI y el ayudante de macOS en modo release y, si se pide, los instala.
# Uso: sh zerlb/herramientas/compilar.sh [--instalar CARPETA]
# Al instalar, guarda los binarios que había en CARPETA/anterior/ para poder volver atrás.

set -eu
cd "$(git rev-parse --show-toplevel)"
cargo build --locked --release -p agent-desktop --bin agent-desktop
cargo build --locked --release -p agent-desktop-macos --bin agent-desktop-macos-helper
echo "Compilado: $(target/release/agent-desktop version)"
echo "Commit: $(git describe --tags --always --dirty)"

if [ "${1:-}" = "--instalar" ]; then
    destino="${2:?Falta la carpeta de destino}"
    mkdir -p "$destino/anterior"
    for archivo in agent-desktop agent-desktop-macos-helper; do
        [ -f "$destino/$archivo" ] && cp -p "$destino/$archivo" "$destino/anterior/$archivo"
        install -m 755 "target/release/$archivo" "$destino/$archivo"
    done
    git describe --tags --always --dirty > "$destino/COMPILACION"
    echo "Instalado en $destino (lo anterior quedó en $destino/anterior/)."
fi
