#!/bin/sh
# Compila el lanzador fijo de `agent-desktop mcp` (lanzador_mcp.c) con las rutas de este equipo.
# Uso: sh zerlb/herramientas/compilar_lanzador_mcp.sh DESTINO BINARIO_REAL REGISTRO_SH
#   DESTINO       archivo del lanzador, fuera de carpetas protegidas (Escritorio, Documentos,
#                 Descargas), por ejemplo ~/Library/Application Support/AgentDesktop/mcp/agent-desktop-mcp
#   BINARIO_REAL  el binario de agent-desktop que se quiere usar
#   REGISTRO_SH   registro.sh, con registro_mcp.py a su lado
# Después, en el cliente MCP: {"command": "DESTINO"} sin argumentos. macOS pedirá permisos para
# el lanzador la primera vez (carpetas, Accesibilidad, Grabación de pantalla). No lo vuelvas a
# compilar si no cambian las rutas: una compilación nueva tiene otra firma y pierde los permisos.

set -eu
destino="${1:?Falta DESTINO}"
real="${2:?Falta BINARIO_REAL}"
registro="${3:?Falta REGISTRO_SH}"
fuente="$(cd "$(dirname "$0")" && pwd)/lanzador_mcp.c"
mkdir -p "$(dirname "$destino")"
cc -O2 -Wall -Wextra -Werror -o "$destino.nuevo" "$fuente" \
    -DREAL="\"$real\"" -DREGISTRO="\"$registro\""
codesign --force --sign - --identifier agent-desktop-mcp "$destino.nuevo"
mv -f "$destino.nuevo" "$destino"
echo "Lanzador listo: $destino"
codesign -dv "$destino" 2>&1 | grep -E 'Identifier|CDHash'
