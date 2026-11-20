"""Prueba de controles que fallaron en paneles web reales: lista desplegable hecha por la
página, botón que copia al portapapeles y deslizador.

Uso: python3 -I controles.py "<App>" [--bin RUTA]

Abre antes controles.html en esa app (por ejemplo con el servidor de LEEME.md). Por cada
control dice si el comando salió bien y qué recibió la página de verdad, leído de su
renglón «Estado», y al final si la app al frente cambió. Guarda el texto del portapapeles
antes de la prueba y lo devuelve al terminar.
"""
import json
import subprocess
import sys
import time

APP = sys.argv[1]
BIN = sys.argv[sys.argv.index("--bin") + 1] if "--bin" in sys.argv else "agent-desktop"


def ad(*args):
    p = subprocess.run([BIN, *args], capture_output=True, text=True)
    try:
        return json.loads(p.stdout)
    except ValueError:
        return {"ok": False, "error": {"code": "SIN_JSON", "message": (p.stdout + p.stderr)[:200]}}


def frente():
    asn = subprocess.run(["lsappinfo", "front"], capture_output=True, text=True).stdout.strip()
    info = subprocess.run(["lsappinfo", "info", "-only", "name", asn], capture_output=True, text=True).stdout
    return info.split("=", 1)[-1].strip().strip('"')


def ventana():
    ws = ad("list-windows", "--app", APP).get("data") or []
    for w in ws if isinstance(ws, list) else ws.get("windows", []):
        if (w.get("title") or "").startswith("Controles agent-desktop"):
            return w["id"]
    return None


def leer(wid):
    d = ad("snapshot", "--app", APP, "--window-id", wid, "-i", "--compact")
    refs, estado = {}, ""

    def rec(n):
        nonlocal estado
        if isinstance(n, dict):
            nombre = n.get("name") or n.get("description") or ""
            if n.get("ref_id") and nombre:
                refs.setdefault((n.get("role"), nombre), n["ref_id"])
            texto = str(n.get("name") or n.get("value") or "")
            if texto.startswith("Estado:"):
                estado = texto
            for c in n.get("children") or []:
                rec(c)

    rec((d.get("data") or {}).get("tree"))
    return refs, estado


def estado(wid):
    d = ad("snapshot", "--app", APP, "--window-id", wid, "--compact")
    hallado = [""]

    def rec(n):
        if isinstance(n, dict):
            texto = str(n.get("name") or n.get("value") or "")
            if texto.startswith("Estado:"):
                hallado[0] = texto
            for c in n.get("children") or []:
                rec(c)

    rec((d.get("data") or {}).get("tree"))
    return hallado[0]


def resumen(d):
    if d.get("ok"):
        return "ok " + ((d.get("data") or {}).get("disposition") or {}).get("delivery", "")
    e = d.get("error") or {}
    return f"{e.get('code')} {str(e.get('message'))[:60]}"


def main():
    antes = frente()
    wid = ventana()
    if not wid:
        print("No encontré la ventana de controles en", APP)
        sys.exit(1)
    portapapeles = subprocess.run(["pbpaste"], capture_output=True).stdout
    refs, _ = leer(wid)
    bien = 0

    combo = refs.get(("combobox", "Industria"))
    d = ad("select", combo, "Salud") if combo else {"error": {"code": "SIN_REF"}}
    time.sleep(0.3)
    recibio = "industria=Salud" in estado(wid)
    bien += recibio
    print(f"  Lista desplegable  select     {resumen(d):34} la página recibió: {'sí' if recibio else 'NO'}")

    refs, _ = leer(wid)
    boton = refs.get(("button", "Copiar clave"))
    d = ad("click", boton) if boton else {"error": {"code": "SIN_REF"}}
    time.sleep(0.6)
    copiado = subprocess.run(["pbpaste"], capture_output=True, text=True).stdout.startswith("clave-copiada-")
    recibio = "copiar=ok" in estado(wid) and copiado
    bien += recibio
    print(f"  Botón Copiar       click      {resumen(d):34} llegó al portapapeles: {'sí' if recibio else 'NO'}")

    refs, _ = leer(wid)
    barra = refs.get(("slider", "Grosor de la línea"))
    d = ad("set-value", barra, "7") if barra else {"error": {"code": "SIN_REF"}}
    time.sleep(0.3)
    recibio = "grosor=7" in estado(wid)
    bien += recibio
    print(f"  Deslizador         set-value  {resumen(d):34} la página recibió: {'sí' if recibio else 'NO'}")

    subprocess.run(["pbcopy"], input=portapapeles)
    print(f"  {bien} de 3 · al frente: {antes} → {frente()}")


main()
