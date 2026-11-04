"""Prueba de registro_mcp.py con un servidor MCP falso, sin pantalla ni binario real.

Comprueba que los mensajes pasan intactos en los dos sentidos, que cada llamada de
herramienta deja una línea con los mismos campos que registro.sh, que los errores y las
cancelaciones se anotan, y que nunca se guardan textos, valores ni referencias.

Uso: python3 -I zerlb/herramientas/prueba_registro_mcp.py
"""

import json
import os
import subprocess
import sys
import tempfile

AQUI = os.path.dirname(os.path.abspath(__file__))
SECRETO = "texto-privado-que-no-debe-guardarse"

SERVIDOR_FALSO = r'''
import json, sys
for linea in sys.stdin:
    m = json.loads(linea)
    if m.get("method") == "initialize":
        r = {"jsonrpc": "2.0", "id": m["id"], "result": {"protocolVersion": "2025-11-25"}}
    elif m.get("method") == "tools/call":
        nombre = m["params"]["name"]
        if nombre == "desktop_type":
            sobre = {"version": "2.6", "ok": False, "command": "type",
                     "error": {"code": "INVALID_ARGS", "message": "bad value 'x' at 3"}}
        elif nombre == "desktop_wait":
            continue
        elif nombre == "desktop_bad":
            sys.stdout.write(json.dumps({"jsonrpc": "2.0", "id": m["id"], "result": {"isError": True,
                "content": [{"type": "text", "text": "Unknown argument 'x9' for desktop_bad. Known arguments: ref_id"}]}}) + "\n")
            sys.stdout.flush()
            continue
        else:
            sobre = {"version": "2.6", "ok": True, "command": "click",
                     "data": {"text": sys.argv[1], "delivery": "delivered_verified",
                              "steps": [{"label": "AXPress"}]}}
        r = {"jsonrpc": "2.0", "id": m["id"], "result": {
            "content": [{"type": "text", "text": json.dumps(sobre)}],
            "structuredContent": sobre, "isError": not sobre["ok"]}}
    else:
        continue
    sys.stdout.write(json.dumps(r) + "\n")
    sys.stdout.flush()
'''


def main():
    fallas = []
    with tempfile.TemporaryDirectory() as carpeta:
        falso = os.path.join(carpeta, "servidor.py")
        with open(falso, "w", encoding="utf-8") as archivo:
            archivo.write(SERVIDOR_FALSO)
        entorno = dict(os.environ, AGENT_DESKTOP_USO=os.path.join(carpeta, "uso"),
                       AGENT_DESKTOP_HOME=os.path.join(carpeta, "home"))
        pedidos = [
            {"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {}},
            {"jsonrpc": "2.0", "id": 2, "method": "tools/call", "params": {
                "name": "desktop_click", "arguments": {"ref_id": "@ab12cd:e7", "app": "Calculadora"}}},
            {"jsonrpc": "2.0", "id": "tres", "method": "tools/call", "params": {
                "name": "desktop_type", "arguments": {"ref_id": "e3", "text": SECRETO}}},
            {"jsonrpc": "2.0", "id": 4, "method": "tools/call", "params": {
                "name": "desktop_wait", "arguments": {"text": SECRETO}}},
            {"jsonrpc": "2.0", "method": "notifications/cancelled", "params": {"requestId": 4}},
            {"jsonrpc": "2.0", "id": 5, "method": "tools/call", "params": {
                "name": "desktop_bad", "arguments": {"x9": SECRETO}}},
        ]
        entrada = "".join(json.dumps(p) + "\n" for p in pedidos)
        proceso = subprocess.run(
            [sys.executable, "-I", os.path.join(AQUI, "registro_mcp.py"), sys.executable, falso, SECRETO,
             "--headed"],
            input=entrada.encode(), stdout=subprocess.PIPE, env=entorno, timeout=30)
        respuestas = [json.loads(l) for l in proceso.stdout.decode().splitlines()]
        if [r["id"] for r in respuestas] != [1, 2, "tres", 5]:
            fallas.append("las respuestas no pasaron intactas: %r" % [r.get("id") for r in respuestas])
        if respuestas and respuestas[1]["result"]["structuredContent"]["data"]["text"] != SECRETO:
            fallas.append("el intermediario cambió el contenido de una respuesta")
        registro = os.path.join(carpeta, "uso", "registro.jsonl")
        with open(registro, encoding="utf-8") as archivo:
            texto = archivo.read()
        lineas = [json.loads(l) for l in texto.splitlines()]
        if SECRETO in texto or "@ab12cd" in texto or "ab12cd" in texto:
            fallas.append("el registro guardó un texto o una referencia")
        por_cmd = {l["cmd"]: l for l in lineas}
        esperado = {
            "click": {"ok": True, "codigo": "", "app": "Calculadora", "ref": "completa", "fisico": True,
                      "metodo": "AXPress", "entrega": "delivered_verified", "via": "mcp",
                      "opciones": "--ref-id,--app"},
            "type": {"ok": False, "codigo": "INVALID_ARGS", "ref": "corta", "detalle": "bad value '_' at "},
            "wait": {"ok": False, "codigo": "CANCELLED"},
            "bad": {"ok": False, "codigo": "TOOL_ERROR", "opciones": "--x9",
                    "detalle": "Unknown argument '_' for desktop_bad. Known arguments: ref_id"},
            "mcp": {"rc": 0, "via": "mcp"},
        }
        for cmd, campos in esperado.items():
            linea = por_cmd.get(cmd)
            if linea is None:
                fallas.append("falta la línea de %s" % cmd)
                continue
            for campo, valor in campos.items():
                if linea.get(campo) != valor:
                    fallas.append("%s.%s = %r, se esperaba %r" % (cmd, campo, linea.get(campo), valor))
        if len(lineas) != 5:
            fallas.append("se esperaban 5 líneas y hay %d" % len(lineas))
    for falla in fallas:
        print("FALLA:", falla)
    print("registro_mcp: %s" % ("bien" if not fallas else "%d fallas" % len(fallas)))
    return 1 if fallas else 0


if __name__ == "__main__":
    sys.exit(main())
