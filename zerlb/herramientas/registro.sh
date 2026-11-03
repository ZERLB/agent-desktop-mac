#!/bin/zsh
# Envoltorio que mide el uso de agent-desktop sin guardar lo que hay en pantalla.
#
# Instálalo con el nombre "agent-desktop" en tu ruta de comandos y dile dónde está el
# binario real con AGENT_DESKTOP_REAL. Por cada llamada agrega una línea a
# $AGENT_DESKTOP_USO/registro.jsonl (por omisión ~/.agent-desktop-uso) con: fecha,
# comando, si fue físico (--headed), si salió bien, el código de error, los
# milisegundos y el nombre de la app de --app. Nunca guarda textos, referencias,
# valores, capturas ni la salida del comando. Si el registro falla, la llamada no.

REAL="${AGENT_DESKTOP_REAL:?Define AGENT_DESKTOP_REAL con la ruta del binario real de agent-desktop}"
DIR="${AGENT_DESKTOP_USO:-$HOME/.agent-desktop-uso}"

zmodload zsh/datetime
inicio=$EPOCHREALTIME
salida=$("$REAL" "$@")
rc=$?
printf '%s\n' "$salida"

{
    printf -v ms '%.0f' $(( (EPOCHREALTIME - inicio) * 1000 ))
    cmd=""
    fisico=false
    app=""
    previo=""
    for arg in "$@"; do
        case "$arg" in --headed) fisico=true ;; esac
        [ "$previo" = "--app" ] && app="$arg"
        if [ -z "$cmd" ]; then
            case "$previo" in
                --session | --agent-id | --trace | --screenshot | --app | --window-id) ;;
                *) case "$arg" in -*) ;; *) cmd="$arg" ;; esac ;;
            esac
        fi
        previo="$arg"
    done
    # El sobre JSON lleva "ok" al principio (versión 2.6) o al final (2.5).
    ok=$( { printf '%s' "$salida" | head -c 300; printf '%s' "$salida" | tail -c 300; } \
        | grep -o '"ok":\(true\|false\)' | head -1 | cut -d: -f2)
    [ -z "$ok" ] && ok=false
    codigo=""
    if [ "$ok" = "false" ]; then
        codigo=$(printf '%s' "$salida" | grep -o '"code":"[A-Z_]*"' | head -1 | cut -d'"' -f4)
    fi
    limpiar() { printf '%s' "$1" | tr -d '"\\\n\r' | cut -c1-80; }
    mkdir -p "$DIR"
    printf '{"t":"%s","cmd":"%s","fisico":%s,"ok":%s,"codigo":"%s","ms":%s,"rc":%s,"app":"%s"}\n' \
        "$(date -u +%Y-%m-%dT%H:%M:%SZ)" "$(limpiar "$cmd")" "$fisico" "$ok" "$codigo" \
        "$ms" "$rc" "$(limpiar "$app")" >> "$DIR/registro.jsonl"
} 2>/dev/null

exit "$rc"
