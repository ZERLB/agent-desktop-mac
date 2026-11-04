"""Mide el uso de `agent-desktop mcp` sin guardar lo que hay en pantalla.

registro.sh retiene la salida de cada comando hasta el final, y el servidor MCP habla por la
entrada y la salida estándar durante toda la sesión; además corre cada herramienta con el
binario real, no con el lanzador. Este intermediario arranca el servidor, pasa cada mensaje
tal cual en los dos sentidos y, por cada llamada de herramienta (`tools/call`), agrega a
$AGENT_DESKTOP_USO/registro.jsonl (por omisión ~/.agent-desktop-uso) una línea con los mismos
campos que registro.sh y "via":"mcp": comando, si fue físico (servidor con --headed), si salió
bien, código de error, milisegundos, app, nombres de los argumentos, forma de la referencia,
vía y entrega de la acción. Al cerrar el servidor anota una línea "mcp" con su duración.
Nunca guarda textos, referencias, valores, capturas ni la salida. Si el registro falla, el
servidor sigue igual.

Uso: python3 -I registro_mcp.py BINARIO_REAL mcp [opciones globales]
"""

import datetime
import glob
import json
import os
import re
import subprocess
import sys
import threading
import time

DIRECTORIO = os.environ.get("AGENT_DESKTOP_USO") or os.path.expanduser("~/.agent-desktop-uso")
REF_COMPLETA = re.compile(r"^@[a-z0-9]+:e[0-9]+$")
REF_CORTA = re.compile(r"^@?e[0-9]+$")
REF_VACIADA = re.compile(r"^@[0-9]+$")
METODO = re.compile(r"^[A-Za-z_ ]{1,40}$")
ENTREGA = re.compile(r"^[a-z_]+$")
CODIGO = re.compile(r"^[A-Z_]+$")

candado = threading.Lock()
pendientes = {}


def limpiar(texto):
    texto = str(texto or "")
    for caracter in '"\\\n\r':
        texto = texto.replace(caracter, "")
    return texto[:80]


def ahora():
    return datetime.datetime.now(datetime.timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ")


def anotar(linea):
    try:
        os.makedirs(DIRECTORIO, exist_ok=True)
        with open(os.path.join(DIRECTORIO, "registro.jsonl"), "a", encoding="utf-8") as archivo:
            archivo.write(json.dumps(linea, ensure_ascii=False, separators=(",", ":")) + "\n")
    except Exception:
        pass


def app_de_lectura(lectura):
    raiz = os.environ.get("AGENT_DESKTOP_HOME") or os.path.expanduser("~/.agent-desktop")
    candidatos = [os.path.join(raiz, "snapshots", lectura, "refmap.json")]
    candidatos += glob.glob(os.path.join(raiz, "sessions", "*", "snapshots", lectura, "refmap.json"))
    for candidato in candidatos:
        try:
            with open(candidato, encoding="utf-8") as archivo:
                encontrado = re.search(r'"source_app":"([^"]*)"', archivo.read())
            if encontrado:
                return encontrado.group(1)
        except OSError:
            continue
    return ""


def forma_de_referencias(argumentos):
    forma, lectura = "", ""
    for valor in argumentos.values():
        if not isinstance(valor, str):
            continue
        if REF_COMPLETA.match(valor):
            forma = forma or "completa"
            lectura = lectura or valor[1:].split(":", 1)[0]
        elif REF_CORTA.match(valor):
            forma = "corta"
        elif REF_VACIADA.match(valor):
            forma = "vaciada-por-zsh"
    return forma, lectura


def primero(objeto, clave, patron):
    """Busca en el sobre JSON el primer valor de `clave` que cumpla `patron`."""
    pila = [objeto]
    while pila:
        actual = pila.pop(0)
        if isinstance(actual, dict):
            valor = actual.get(clave)
            if isinstance(valor, str) and patron.match(valor):
                return valor
            pila.extend(actual.values())
        elif isinstance(actual, list):
            pila.extend(actual)
    return ""


def metodo_de(sobre):
    pila = [sobre]
    while pila:
        actual = pila.pop(0)
        if isinstance(actual, dict):
            pasos = actual.get("steps")
            if isinstance(pasos, list):
                for paso in pasos:
                    etiqueta = paso.get("label") if isinstance(paso, dict) else None
                    if isinstance(etiqueta, str) and METODO.match(etiqueta):
                        return etiqueta
            pila.extend(actual.values())
        elif isinstance(actual, list):
            pila.extend(actual)
    return ""


def sin_valores(mensaje):
    if not isinstance(mensaje, str):
        return ""
    mensaje = re.sub(r"'[^']*'", "'_'", mensaje)
    return re.sub(r"[^A-Za-z_<>:@' .,()-]", "", mensaje)[:100]


def detalle_de(sobre):
    error = sobre.get("error") if isinstance(sobre, dict) else None
    return sin_valores(error.get("message") if isinstance(error, dict) else "")


def texto_de(resultado):
    contenido = resultado.get("content") if isinstance(resultado, dict) else None
    if isinstance(contenido, list) and contenido and isinstance(contenido[0], dict):
        return contenido[0].get("text") or ""
    return ""


def al_pedir(mensaje, fisico):
    metodo = mensaje.get("method")
    if metodo == "tools/call" and "id" in mensaje:
        parametros = mensaje.get("params") or {}
        argumentos = parametros.get("arguments") or {}
        if not isinstance(argumentos, dict):
            argumentos = {}
        nombre = str(parametros.get("name") or "")
        if nombre.startswith("desktop_"):
            nombre = nombre[len("desktop_"):]
        forma, lectura = forma_de_referencias(argumentos)
        app = argumentos.get("app") if isinstance(argumentos.get("app"), str) else ""
        with candado:
            pendientes[json.dumps(mensaje["id"])] = {
                "inicio": time.monotonic(),
                "cmd": nombre.replace("_", "-"),
                "fisico": fisico,
                "app": app,
                "lectura": lectura,
                "opciones": ",".join(
                    "--" + clave.replace("_", "-") for clave, valor in argumentos.items() if valor is not None
                ),
                "ref": forma,
            }
    elif metodo == "notifications/cancelled":
        solicitud = (mensaje.get("params") or {}).get("requestId")
        cerrar(json.dumps(solicitud), None, cancelada=True)


def cerrar(clave, respuesta, cancelada=False):
    with candado:
        llamada = pendientes.pop(clave, None)
    if llamada is None:
        return
    sobre, ok, codigo, detalle = {}, False, "", ""
    if cancelada:
        codigo = "CANCELLED"
    elif "error" in respuesta:
        codigo = "MCP_ERROR"
    else:
        resultado = respuesta.get("result") or {}
        sobre = resultado.get("structuredContent") if isinstance(resultado, dict) else None
        if isinstance(sobre, dict) and isinstance(sobre.get("ok"), bool):
            ok = sobre["ok"]
        else:
            sobre = {}
            ok = not (isinstance(resultado, dict) and resultado.get("isError"))
            if not ok:
                codigo, detalle = "TOOL_ERROR", sin_valores(texto_de(resultado))
        if not ok and not codigo:
            codigo = primero(sobre.get("error") or {}, "code", CODIGO)
        if codigo == "INVALID_ARGS":
            detalle = detalle_de(sobre)
    app = llamada["app"] or (app_de_lectura(llamada["lectura"]) if llamada["lectura"] else "")
    anotar({
        "t": ahora(),
        "cmd": limpiar(llamada["cmd"]),
        "fisico": llamada["fisico"],
        "ok": ok,
        "codigo": codigo,
        "ms": round((time.monotonic() - llamada["inicio"]) * 1000),
        "rc": 0 if ok else 1,
        "app": limpiar(app),
        "opciones": limpiar(llamada["opciones"]),
        "ref": llamada["ref"],
        "ayuda": False,
        "detalle": limpiar(detalle),
        "metodo": limpiar(metodo_de(sobre)),
        "entrega": primero(sobre, "delivery", ENTREGA),
        "via": "mcp",
    })


def mensajes(linea):
    try:
        datos = json.loads(linea)
    except ValueError:
        return []
    if isinstance(datos, list):
        return [dato for dato in datos if isinstance(dato, dict)]
    return [datos] if isinstance(datos, dict) else []


def hacia_el_servidor(servidor, fisico):
    entrada = sys.stdin.buffer
    try:
        for linea in iter(entrada.readline, b""):
            try:
                for mensaje in mensajes(linea):
                    al_pedir(mensaje, fisico)
            except Exception:
                pass
            servidor.stdin.write(linea)
            servidor.stdin.flush()
    except (BrokenPipeError, OSError):
        pass
    finally:
        try:
            servidor.stdin.close()
        except OSError:
            pass


def main():
    if len(sys.argv) < 3:
        sys.stderr.write("Uso: registro_mcp.py BINARIO_REAL mcp [opciones globales]\n")
        return 2
    inicio = time.monotonic()
    fisico = "--headed" in sys.argv[2:]
    servidor = subprocess.Popen(sys.argv[1:], stdin=subprocess.PIPE, stdout=subprocess.PIPE)
    threading.Thread(target=hacia_el_servidor, args=(servidor, fisico), daemon=True).start()
    salida = sys.stdout.buffer
    try:
        for linea in iter(servidor.stdout.readline, b""):
            salida.write(linea)
            salida.flush()
            try:
                for mensaje in mensajes(linea):
                    if "id" in mensaje and ("result" in mensaje or "error" in mensaje):
                        cerrar(json.dumps(mensaje["id"]), mensaje)
            except Exception:
                pass
    except (BrokenPipeError, OSError):
        servidor.terminate()
    rc = servidor.wait()
    anotar({
        "t": ahora(), "cmd": "mcp", "fisico": fisico, "ok": rc == 0, "codigo": "",
        "ms": round((time.monotonic() - inicio) * 1000), "rc": rc, "app": "", "opciones": "",
        "ref": "", "ayuda": False, "detalle": "", "metodo": "", "entrega": "", "via": "mcp",
    })
    return rc


if __name__ == "__main__":
    sys.exit(main())
