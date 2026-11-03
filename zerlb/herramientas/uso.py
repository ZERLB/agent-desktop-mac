"""Resumen del uso de agent-desktop a partir del registro de registro.sh.

Uso: python3 uso.py [--dias 7] [--registro RUTA] [--md]

Dice qué comandos se usan, cuántos fallan y con qué código, cuánto tardan y en qué
apps. Sirve para decidir qué arreglar primero: lo que más se usa y más falla.
"""

import argparse
import collections
import datetime
import json
import os
import statistics
import sys


def leer(ruta, desde):
    filas = []
    try:
        with open(ruta, encoding="utf-8") as archivo:
            for linea in archivo:
                try:
                    fila = json.loads(linea)
                    t = datetime.datetime.strptime(fila["t"], "%Y-%m-%dT%H:%M:%SZ")
                except (ValueError, KeyError):
                    continue
                if t >= desde:
                    filas.append(fila)
    except FileNotFoundError:
        pass
    return filas


def percentil(valores, p):
    if not valores:
        return 0
    orden = sorted(valores)
    return orden[min(len(orden) - 1, int(round(p / 100 * (len(orden) - 1))))]


def resumir(filas, dias, md):
    fuera = []
    titulo = f"Uso de agent-desktop · últimos {dias} días"
    fuera.append(f"## {titulo}" if md else titulo)
    ayudas = sum(1 for f in filas if f.get("ayuda"))
    filas = [f for f in filas if not f.get("ayuda")]
    if not filas:
        fuera.append("Sin llamadas registradas en ese periodo." + (f" Consultas de ayuda: {ayudas}." if ayudas else ""))
        return "\n".join(fuera)
    total = len(filas)
    malas = [f for f in filas if not f.get("ok")]
    fisicas = sum(1 for f in filas if f.get("fisico"))
    fuera.append(
        f"Llamadas: {total} · con error: {len(malas)} ({100 * len(malas) // total} %) · "
        f"modo físico: {fisicas} · de {filas[0]['t'][:10]} a {filas[-1]['t'][:10]}"
        + (f" · consultas de ayuda aparte: {ayudas}" if ayudas else "")
    )
    por_cmd = collections.defaultdict(list)
    for fila in filas:
        por_cmd[fila.get("cmd") or "(sin comando)"].append(fila)
    fuera.append("")
    fuera.append("| Comando | Llamadas | Con error | Mediana ms | p95 ms | Errores más comunes |" if md
                 else "Comando · llamadas · con error · mediana ms · p95 ms · errores")
    if md:
        fuera.append("| --- | ---: | ---: | ---: | ---: | --- |")
    for cmd, grupo in sorted(por_cmd.items(), key=lambda par: -len(par[1])):
        ms = [f.get("ms", 0) for f in grupo]
        errores = collections.Counter(f.get("codigo") or "?" for f in grupo if not f.get("ok"))
        comunes = ", ".join(f"{codigo} ×{n}" for codigo, n in errores.most_common(3)) or "—"
        fallas = sum(errores.values())
        fila = (cmd, len(grupo), fallas, int(statistics.median(ms)), percentil(ms, 95), comunes)
        fuera.append("| " + " | ".join(str(x) for x in fila) + " |" if md else " · ".join(str(x) for x in fila))
    apps = collections.Counter(f.get("app") for f in filas if f.get("app"))
    if apps:
        fuera.append("")
        fuera.append("Apps más usadas: " + ", ".join(f"{a} ×{n}" for a, n in apps.most_common(10)))
    if malas:
        codigos = collections.Counter(f.get("codigo") or "?" for f in malas)
        fuera.append("Errores en total: " + ", ".join(f"{c} ×{n}" for c, n in codigos.most_common()))
    refs = collections.Counter(f.get("ref") for f in filas if f.get("ref") and f.get("ref") != "completa")
    if refs:
        fuera.append("Referencias mal escritas: " + ", ".join(f"{forma} ×{n}" for forma, n in refs.most_common())
                     + " (vaciada-por-zsh: se escribió \"@$VAR:eN\" sin llaves)")
    vias = collections.defaultdict(collections.Counter)
    for f in filas:
        if f.get("metodo"):
            clave = (f.get("cmd") or "?", f.get("app") or "sin app")
            vias[clave][(f["metodo"], f.get("entrega") or "?")] += 1
    if vias:
        fuera.append("")
        fuera.append("Vía de entrega por comando y app (vía · entrega ×veces):")
        for (cmd, app), cuenta in sorted(vias.items(), key=lambda par: -sum(par[1].values()))[:10]:
            fuera.append(f"- {cmd} en {app}: " + ", ".join(f"{m} · {e} ×{n}" for (m, e), n in cuenta.most_common(3)))
    argumentos = collections.Counter(
        (f.get("cmd") or "?", f.get("detalle") or "?", f.get("opciones") or "sin opciones")
        for f in malas if f.get("codigo") == "INVALID_ARGS" and "detalle" in f)
    if argumentos:
        fuera.append("")
        fuera.append("Argumentos rechazados (comando · mensaje · opciones usadas):")
        for (cmd, detalle, opciones), n in argumentos.most_common(8):
            fuera.append(f"- {cmd} · {detalle} · {opciones} ×{n}")
    return "\n".join(fuera)


def main():
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--dias", type=int, default=7)
    parser.add_argument("--registro", default=os.path.join(
        os.environ.get("AGENT_DESKTOP_USO", os.path.expanduser("~/.agent-desktop-uso")), "registro.jsonl"))
    parser.add_argument("--md", action="store_true", help="salida en Markdown")
    args = parser.parse_args()
    desde = datetime.datetime.now(datetime.timezone.utc).replace(tzinfo=None) - datetime.timedelta(days=args.dias)
    print(resumir(leer(args.registro, desde), args.dias, args.md))


if __name__ == "__main__":
    sys.exit(main())
