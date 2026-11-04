"""Prueba de type en editores web: contenteditable simple, editor controlado, campo controlado y chat.

Uso: python3 -I editores.py "<App>" [--bin RUTA] [--texto "Hola, ¿qué tal? ñ 😀"]

Abre antes editores.html en esa app (por ejemplo con el servidor de LEEME.md). Por cada
campo dice si type salió bien, con qué método y cuánto tardó; al final lee lo que la página
recibió de verdad, no sólo lo que se ve, y si la app al frente cambió. En el chat (Enter
envía, Shift+Enter agrega una línea) escribe dos líneas, comprueba que no se envió nada y
después lo envía con press --app.
"""
import json
import subprocess
import sys
import time

APP = sys.argv[1]
BIN = sys.argv[sys.argv.index("--bin") + 1] if "--bin" in sys.argv else "agent-desktop"
TEXTO = sys.argv[sys.argv.index("--texto") + 1] if "--texto" in sys.argv else "Hola, ¿qué tal? ñ 😀"
CAMPOS = ("Editor simple", "Editor controlado", "Campo controlado")


def ad(*args):
    p = subprocess.run([BIN, *args], capture_output=True, text=True)
    try:
        return json.loads(p.stdout)
    except ValueError:
        return {"ok": False, "error": {"code": "SIN_JSON", "message": (p.stdout + p.stderr)[:200]}}


def frente():
    return subprocess.run(
        ["osascript", "-e", 'tell application "System Events" to get name of first application process whose frontmost is true'],
        capture_output=True, text=True).stdout.strip()


def ventana():
    d = ad("list-windows", "--app", APP)
    for w in d.get("data") or []:
        if "Editores" in (w.get("title") or ""):
            return w["id"]
    return None


def leer(wid):
    d = ad("snapshot", "--app", APP, "--window-id", wid, "--compact")
    refs, estado = {}, None

    def rec(n):
        nonlocal estado
        if n.get("ref_id") and n.get("name") and n.get("role") == "textfield":
            refs.setdefault(n["name"], n["ref_id"])
        for clave in ("name", "value"):
            valor = n.get(clave)
            if isinstance(valor, str) and valor.startswith("Simple:"):
                estado = valor
        for hijo in n.get("children") or []:
            rec(hijo)
    if d.get("ok"):
        rec(d["data"]["tree"])
    return refs, estado


def main():
    wid = ventana()
    if not wid:
        print("No encontré la ventana «Editores agent-desktop» en", APP)
        return 1
    antes = frente()
    refs, _ = leer(wid)
    for campo in CAMPOS:
        if campo not in refs:
            print(f"  {campo:<18} SIN REFERENCIA (¿la página cargó con la ventana a la vista?)")
            continue
        inicio = time.time()
        d = ad("type", refs[campo], TEXTO)
        ms = int((time.time() - inicio) * 1000)
        if d.get("ok"):
            metodo = ((d.get("data") or {}).get("steps") or [{}])[0].get("label")
        else:
            metodo = d["error"]["code"] + " " + str(d["error"].get("message"))[:70]
        print(f"  {campo:<18} ok={d.get('ok')!s:<5} {ms:>5} ms  {metodo}")
    time.sleep(0.8)
    _, estado = leer(wid)
    recibido = dict(linea.split(": ", 1) for linea in (estado or "").splitlines() if ": " in linea)
    for campo, clave in zip(CAMPOS, ("Simple", "Controlado", "Campo")):
        bien = recibido.get(clave, "").endswith(TEXTO)
        print(f"  la página recibió en {campo:<18} {'sí' if bien else 'NO'}")
    if "Chat" in refs:
        sin_enviar, enviado = probar_chat(wid, refs["Chat"])
        print(f"  chat: dos líneas sin enviar {'sí' if sin_enviar else 'NO'} · enviado con Return {'sí' if enviado else 'NO'}")
    print(f"  al frente: {antes} → {frente()}")
    return 0


def lineas_de_estado(wid):
    _, estado = leer(wid)
    return dict(linea.split(": ", 1) for linea in (estado or "").splitlines() if ": " in linea)


def probar_chat(wid, ref):
    ad("type", ref, "línea uno\nlínea dos")
    time.sleep(0.6)
    lineas = lineas_de_estado(wid)
    sin_enviar = lineas.get("Chat", "").endswith("línea uno⏎línea dos") and "línea dos" not in lineas.get("Enviados", "")
    ad("press", "--app", APP, "return")
    time.sleep(0.6)
    lineas = lineas_de_estado(wid)
    enviado = lineas.get("Enviados", "").endswith("[línea uno⏎línea dos]") and not lineas.get("Chat", "").strip()
    return sin_enviar, enviado


sys.exit(main())
