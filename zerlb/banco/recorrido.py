"""Recorrido del banco de pruebas web con agent-desktop, sin pantalla primero.

Uso: python3 -I banco.py "<App>" [--bin RUTA] [--fisico]
Imprime una tabla de pasos: qué método funcionó y qué se leyó después.
"""
import json
import os
import subprocess
import sys
import time

APP = sys.argv[1]
BIN = "agent-desktop"
if "--bin" in sys.argv:
    BIN = sys.argv[sys.argv.index("--bin") + 1]
FISICO = "--fisico" in sys.argv
pasos = []


def ad(*args, fisico=False):
    cmd = [BIN] + (["--headed"] if fisico else []) + list(args)
    t = time.time()
    p = subprocess.run(cmd, capture_output=True, text=True)
    ms = int((time.time() - t) * 1000)
    try:
        d = json.loads(p.stdout)
    except Exception:
        d = {"ok": False, "error": {"code": "SIN_JSON", "message": (p.stdout + p.stderr)[:200]}}
    d["_ms"] = ms
    return d


def frente():
    return subprocess.run(["osascript", "-e", 'tell application "System Events" to get name of first application process whose frontmost is true'], capture_output=True, text=True).stdout.strip()


def puntero():
    return subprocess.run(["osascript", "-l", "JavaScript", "-e", 'ObjC.import("AppKit"); var p=$.NSEvent.mouseLocation; Math.round(p.x)+","+Math.round(p.y)'], capture_output=True, text=True).stdout.strip()


def ventana():
    d = ad("list-windows", "--app", APP)
    ws = d.get("data") or []
    ws = ws if isinstance(ws, list) else ws.get("windows", [])
    for w in ws:
        if "Banco de pruebas" in (w.get("title") or ""):
            return w["id"]
    return None


def lectura(wid):
    d = ad("snapshot", "--app", APP, "--window-id", wid, "-i", "--compact")
    refs = {}

    def rec(n):
        if isinstance(n, dict):
            if n.get("ref_id") and n.get("name"):
                refs.setdefault((n.get("role"), n["name"]), n["ref_id"])
            for c in n.get("children") or []:
                rec(c)
    if d.get("ok"):
        rec(d["data"]["tree"])
    return refs, d


def textos(wid):
    d = ad("snapshot", "--app", APP, "--window-id", wid, "--compact")
    out = []

    def rec(n):
        if isinstance(n, dict):
            if n.get("role") in ("statictext", "text") and (n.get("value") or n.get("name")):
                out.append(str(n.get("value") or n.get("name")))
            for c in n.get("children") or []:
                rec(c)
    if d.get("ok"):
        rec(d["data"]["tree"])
    return out


def resumen(d):
    if d.get("ok"):
        disp = (d.get("data") or {}).get("disposition") or {}
        return "ok " + str(disp.get("delivery") or "")
    e = d.get("error") or {}
    return "ERROR " + str(e.get("code")) + " " + str(e.get("message"))[:90]


def paso(nombre, metodo, d, extra=""):
    pasos.append((nombre, metodo, resumen(d), d.get("_ms"), extra))


def escribir(wid, refs, campo, texto):
    ref = refs.get(("textfield", campo))
    if not ref:
        pasos.append((campo, "-", "SIN REFERENCIA", 0, ""))
        return
    for metodo in ("type", "set-value"):
        d = ad(metodo, ref, texto)
        if d.get("ok"):
            break
        refs2, _ = lectura(wid)
        ref = refs2.get(("textfield", campo), ref)
    if not d.get("ok") and FISICO:
        metodo = "--headed type"
        d = ad("type", ref, texto, fisico=True)
    v = ad("get", ref, "--property", "value")
    paso(campo, metodo, d, "valor=" + repr(((v.get("data") or {}).get("value")))[:40])


def main():
    p0, f0 = puntero(), frente()
    wid = ventana()
    if not wid:
        print("No encontré la ventana del banco en", APP)
        sys.exit(1)
    refs, d = lectura(wid)
    recargar = next((r for (rol, n), r in refs.items() if rol == "button" and ("cargar" in n or "Reload" in n)), None)
    if recargar:
        ad("click", recargar)
        time.sleep(1.5)
        refs, d = lectura(wid)
    paso("lectura", "snapshot -i", d, f"{len(refs)} elementos con nombre" + (" (recargada)" if recargar else ""))
    escribir(wid, refs, "Nombre", "Ana Prueba")
    escribir(wid, refs, "Correo", "ana@ejemplo.com")
    ref = refs.get(("combobox", "Plan"))
    d = ad("select", ref, "Profesional") if ref else {"ok": False, "error": {"code": "SIN_REF"}}
    v = ad("get", ref, "--property", "value") if ref else {}
    paso("Plan", "select", d, "valor=" + repr(((v.get("data") or {}).get("value"))))
    ref = refs.get(("checkbox", "Acepto los términos"))
    d = ad("check", ref)
    paso("Acepto", "check", d)
    ref = refs.get(("radiobutton", "Por teléfono"))
    d = ad("click", ref)
    paso("Por teléfono", "click", d)
    escribir(wid, refs, "Mensaje", "Hola desde agent-desktop")
    d = ad("click", refs.get(("button", "Enviar formulario")))
    time.sleep(0.5)
    t = [x for x in textos(wid) if x.startswith("Enviado")]
    paso("Enviar", "click", d, (t[0] if t else "SIN RESULTADO")[:140])
    d = ad("click", refs.get(("button", "Abrir ventana")))
    time.sleep(0.4)
    refs2, _ = lectura(wid)
    cerrar = refs2.get(("button", "Cerrar ventana"))
    paso("Abrir ventana", "click", d, "aparece «Cerrar ventana»" if cerrar else "NO aparece el botón")
    if cerrar:
        d = ad("click", cerrar)
        time.sleep(0.4)
        paso("Cerrar ventana", "click", d, "ok" if "Ventana cerrada" in textos(wid) else "SIN CAMBIO")
    refs3, _ = lectura(wid)
    fin = refs3.get(("link", "Enlace del final"))
    d = ad("scroll-to", fin)
    paso("Bajar al final", "scroll-to", d)
    d = ad("click", fin)
    time.sleep(0.4)
    paso("Enlace final", "click", d, "ok" if "Llegaste al final" in textos(wid) else "SIN CAMBIO")
    p1, f1 = puntero(), frente()
    print(f"\n== {APP} · binario {BIN}")
    for n, m, r, ms, e in pasos:
        print(f"  {n:<16} {m:<14} {r:<48} {ms:>5} ms  {e}")
    print(f"  puntero: {p0} → {p1} · al frente: {f0} → {f1}")


main()
