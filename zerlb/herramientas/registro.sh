#!/bin/zsh
# Envoltorio que mide el uso de agent-desktop sin guardar lo que hay en pantalla.
#
# Instálalo con el nombre "agent-desktop" en tu ruta de comandos y dile dónde está el
# binario real con AGENT_DESKTOP_REAL. Por cada llamada agrega una línea a
# $AGENT_DESKTOP_USO/registro.jsonl (por omisión ~/.agent-desktop-uso) con: fecha,
# comando, si fue físico (--headed), si salió bien, el código de error, los
# milisegundos, la app (la de --app o, en una acción por referencia, la de la lectura
# de donde salió la referencia), los nombres de las opciones usadas, la forma de la
# referencia (completa, corta o vaciada por zsh), si fue una consulta de ayuda, la vía
# con que se entregó la acción (el nombre del primer paso, como AXPress o
# PIDTargetedUnicodeText) y cómo quedó la entrega (delivered_verified, not_delivered…).
# En los errores INVALID_ARGS guarda además el mensaje sin lo que iba entre comillas
# ni números, para saber qué se escribió mal. Nunca guarda textos, referencias,
# valores, capturas ni la salida del comando. Si el registro falla, la llamada no.

REAL="${AGENT_DESKTOP_REAL:?Define AGENT_DESKTOP_REAL con la ruta del binario real de agent-desktop}"
DIR="${AGENT_DESKTOP_USO:-$HOME/.agent-desktop-uso}"

# El servidor MCP habla por la entrada y la salida estándar toda la sesión y no se puede
# retener su salida: pasa por registro_mcp.py, que anota cada llamada de herramienta.
previo=""
for arg in "$@"; do
    case "$previo" in
        --session | --agent-id | --trace | --screenshot | --wait-for) ;;
        *) case "$arg" in
            -*) ;;
            mcp) exec /usr/bin/python3 -I "${0:A:h}/registro_mcp.py" "$REAL" "$@" ;;
            *) break ;;
        esac ;;
    esac
    previo="$arg"
done

zmodload zsh/datetime
inicio=$EPOCHREALTIME
salida=$("$REAL" "$@")
rc=$?
printf '%s\n' "$salida"

{
    printf -v ms '%.0f' $(( (EPOCHREALTIME - inicio) * 1000 ))
    cmd=""
    fisico=false
    ayuda=false
    app=""
    previo=""
    opciones=""
    ref=""
    lectura=""
    for arg in "$@"; do
        case "$arg" in
            --headed) fisico=true ;;
            -h | --help) ayuda=true ;;
            --app=*) app="${arg#--app=}" ;;
        esac
        [ "$previo" = "--app" ] && app="$arg"
        nombre="${arg%%=*}"
        if [[ "$nombre" =~ '^--?[A-Za-z][A-Za-z0-9-]*$' && ",$opciones," != *",$nombre,"* ]]; then
            opciones="${opciones:+$opciones,}$nombre"
        fi
        if [ -z "$cmd" ]; then
            case "$previo" in
                --session | --agent-id | --trace | --screenshot | --app | --window-id) ;;
                *) case "$arg" in -*) ;; *) cmd="$arg" ;; esac ;;
            esac
        elif [[ "$arg" =~ '^@[a-z0-9]+:e[0-9]+$' ]]; then
            ref="${ref:-completa}"
            lectura="${lectura:-${${arg#@}%%:*}}"
        elif [[ "$arg" =~ '^@?e[0-9]+$' ]]; then
            ref="corta"
        elif [[ "$arg" =~ '^@[0-9]+$' ]]; then
            ref="vaciada-por-zsh"
        fi
        previo="$arg"
    done
    [ "$cmd" = "help" ] && ayuda=true
    if [ -z "$app" ] && [ -n "$lectura" ]; then
        raiz="${AGENT_DESKTOP_HOME:-$HOME/.agent-desktop}"
        mapa=""
        for candidato in "$raiz/snapshots/$lectura/refmap.json" "$raiz"/sessions/*/snapshots/"$lectura"/refmap.json(N); do
            [ -f "$candidato" ] && mapa="$candidato" && break
        done
        [ -n "$mapa" ] && app=$(grep -o -m1 '"source_app":"[^"]*"' "$mapa" | head -1 | cut -d'"' -f4)
    fi
    # El sobre JSON lleva "ok" al principio (versión 2.6) o al final (2.5).
    ok=$( { printf '%s' "$salida" | head -c 300; printf '%s' "$salida" | tail -c 300; } \
        | grep -o '"ok":\(true\|false\)' | head -1 | cut -d: -f2)
    if [ -z "$ok" ]; then
        [ "$rc" = 0 ] && ok=true || ok=false
    fi
    metodo=$(printf '%s' "$salida" | grep -o '"steps":\[[^]]*' | head -1 \
        | grep -o '"label":"[A-Za-z_ ]\{1,40\}"' | head -1 | cut -d'"' -f4)
    entrega=$(printf '%s' "$salida" | grep -o '"delivery":"[a-z_]*"' | head -1 | cut -d'"' -f4)
    codigo=""
    detalle=""
    if [ "$ok" = "false" ]; then
        codigo=$(printf '%s' "$salida" | grep -o '"code":"[A-Z_]*"' | head -1 | cut -d'"' -f4)
        if [ "$codigo" = "INVALID_ARGS" ]; then
            detalle=$(printf '%s' "$salida" | grep -o '"message":"[^"]*"' | head -1 | cut -d'"' -f4 \
                | sed -E "s/'[^']*'/'_'/g" | tr -cd "A-Za-z_<>:@' .,()-" | cut -c1-100)
        fi
    fi
    limpiar() { printf '%s' "$1" | tr -d '"\\\n\r' | cut -c1-80; }
    mkdir -p "$DIR"
    printf '{"t":"%s","cmd":"%s","fisico":%s,"ok":%s,"codigo":"%s","ms":%s,"rc":%s,"app":"%s","opciones":"%s","ref":"%s","ayuda":%s,"detalle":"%s","metodo":"%s","entrega":"%s"}\n' \
        "$(date -u +%Y-%m-%dT%H:%M:%SZ)" "$(limpiar "$cmd")" "$fisico" "$ok" "$codigo" \
        "$ms" "$rc" "$(limpiar "$app")" "$(limpiar "$opciones")" "$ref" "$ayuda" \
        "$(limpiar "$detalle")" "$(limpiar "$metodo")" "$entrega" >> "$DIR/registro.jsonl"
} 2>/dev/null

exit "$rc"
