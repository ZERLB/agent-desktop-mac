"""Misma prueba para cada app con agent-desktop, sin leer contenido: sólo cuenta elementos y
anota si cada paso funcionó (abrir en segundo plano, leer la ventana, los menús, una captura que
se borra en seguida, escribir y borrar en el buscador, cerrar si la abrió la prueba).

Uso: python3 -I apps.py LISTA.txt [salida.jsonl]
LISTA.txt: una app por renglón, con su ruta (/Applications/Safari.app). Agrega " | buscar" al
final del renglón sólo si escribir en su buscador no cambia nada (Ajustes, App Store). Nunca lo
pongas en terminales, editores de bases de datos ni apps de mensajes con cuadro de envío.
"""
import json
import os
import subprocess
import sys
import time

LISTA = sys.argv[1]
SALIDA = sys.argv[2] if len(sys.argv) > 2 else "apps.jsonl"
CAPTURA = os.path.join(os.path.dirname(os.path.abspath(SALIDA)), "captura-tmp.png")
BUSCAR_TEXTO = "prueba"
BUSCAR = ("buscar", "search", "busca")


def ad(*args, espera=40):
    t = time.time()
    try:
        p = subprocess.run(["agent-desktop", *args], capture_output=True, text=True, timeout=espera)
        d = json.loads(p.stdout)
    except subprocess.TimeoutExpired:
        d = {"ok": False, "error": {"code": "SIN_RESPUESTA"}}
    except Exception:
        d = {"ok": False, "error": {"code": "SIN_JSON"}}
    d["_ms"] = int((time.time() - t) * 1000)
    return d


def err(d):
    return (d.get("error") or {}).get("code")


def frente():
    return subprocess.run(["osascript", "-e", 'tell application "System Events" to get name of first application process whose frontmost is true'],
                          capture_output=True, text=True).stdout.strip()


def corriendo(bundle):
    d = ad("list-apps")
    apps = d.get("data") or []
    apps = apps if isinstance(apps, list) else apps.get("apps", [])
    return any(a.get("bundle_id") == bundle for a in apps)


def contar(arbol):
    total, roles, buscadores = 0, {}, []

    def rec(n):
        nonlocal total
        if isinstance(n, dict):
            if n.get("ref_id"):
                total += 1
                roles[n.get("role")] = roles.get(n.get("role"), 0) + 1
                nombre = (n.get("name") or "").lower()
                if n.get("role") in ("textfield", "searchfield", "combobox") and any(b in nombre for b in BUSCAR):
                    buscadores.append(n["ref_id"])
            for c in n.get("children") or []:
                rec(c)
    rec(arbol)
    return total, roles, buscadores


def probar(ruta, escribir):
    r = {"app": os.path.basename(ruta)[:-4]}
    try:
        bundle = subprocess.run(["defaults", "read", os.path.join(ruta, "Contents/Info.plist"), "CFBundleIdentifier"],
                                capture_output=True, text=True).stdout.strip()
    except Exception:
        bundle = ""
    if not bundle:
        r["nota"] = "no instalada"
        return r
    r["bundle"] = bundle
    antes = corriendo(bundle)
    r["estaba_abierta"] = antes
    f0 = frente()
    if not antes:
        d = ad("launch", bundle, espera=60)
        r["abrir"] = "ok" if d.get("ok") else err(d)
        r["abrir_ms"] = d["_ms"]
        time.sleep(3)
    snap = None
    for intento in range(3):
        snap = ad("snapshot", "--app", bundle, "-i", "--compact", espera=60)
        if snap.get("ok") or err(snap) not in ("WINDOW_NOT_FOUND", "APP_NOT_FOUND"):
            break
        time.sleep(2)
    if not snap.get("ok") and err(snap) == "WINDOW_NOT_FOUND" and not antes:
        d = ad("launch", bundle, "--activate", espera=60)
        r["necesito_activate"] = bool(d.get("ok"))
        time.sleep(2)
        snap = ad("snapshot", "--app", bundle, "-i", "--compact", espera=60)
    r["leer"] = "ok" if snap.get("ok") else err(snap)
    r["leer_ms"] = snap["_ms"]
    if snap.get("ok"):
        total, roles, buscadores = contar(snap["data"]["tree"])
        r["elementos"] = total
        r["tipos"] = dict(sorted(roles.items(), key=lambda x: -x[1])[:5])
        r["completo"] = snap["data"].get("complete")
        r["buscador"] = bool(buscadores)
        if escribir and buscadores:
            ref = buscadores[0]
            d = ad("type", ref, BUSCAR_TEXTO)
            v = ad("get", ref, "--property", "value")
            valor = (v.get("data") or {}).get("value")
            r["escribir"] = ("ok" if d.get("ok") else err(d)) + f" (valor={valor!r})"
            if not d.get("ok") or valor != "prueba":
                d2 = ad("set-value", ref, "prueba")
                v = ad("get", ref, "--property", "value")
                r["set_value"] = ("ok" if d2.get("ok") else err(d2)) + f" (valor={(v.get('data') or {}).get('value')!r})"
            c = ad("set-value", ref, "")
            if not c.get("ok"):
                c = ad("clear", ref)
            v = ad("get", ref, "--property", "value")
            r["borrar"] = ("ok" if c.get("ok") else err(c)) + f" (valor={(v.get('data') or {}).get('value')!r})"
    menu = ad("snapshot", "--app", bundle, "--surface", "menubar", "-i", "--compact", espera=60)
    if menu.get("ok"):
        arriba = [c.get("name") for c in (menu["data"]["tree"].get("children") or []) if c.get("name")]
        r["menus"] = len(arriba)
    else:
        r["menus"] = err(menu)
    cap = ad("screenshot", "--app", bundle, CAPTURA, espera=60)
    r["captura"] = "ok" if cap.get("ok") and os.path.exists(CAPTURA) else err(cap)
    if os.path.exists(CAPTURA):
        os.remove(CAPTURA)
    r["frente_antes_despues"] = f"{f0} → {frente()}"
    if not antes:
        if bundle == "com.apple.mail":
            r["cerrar"] = "Mail se deja abierto: ciérralo con «Salir y mantener ventanas» para no tocar borradores"
        else:
            d = ad("close-app", bundle, espera=60)
            r["cerrar"] = "ok" if d.get("ok") else err(d)
    return r


def main():
    with open(SALIDA, "w", encoding="utf-8") as out:
        apps = []
        for renglon in open(LISTA, encoding="utf-8"):
            renglon = renglon.strip()
            if renglon and not renglon.startswith("#"):
                ruta, _, marca = renglon.partition("|")
                apps.append((os.path.expanduser(ruta.strip()), marca.strip() == "buscar"))
        for ruta, escribir in apps:
            if not os.path.exists(ruta):
                continue
            try:
                r = probar(ruta, escribir)
            except Exception as e:
                r = {"app": os.path.basename(ruta), "falla_de_la_prueba": str(e)[:200]}
            out.write(json.dumps(r, ensure_ascii=False) + "\n")
            out.flush()
            print(json.dumps(r, ensure_ascii=False), flush=True)


main()
