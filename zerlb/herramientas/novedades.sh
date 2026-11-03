#!/bin/sh
# Dice qué cambió en el agent-desktop original (vercel-labs) desde la última vez que se
# juntó con esta copia, y si hay un lanzamiento nuevo. No cambia nada: sólo consulta.
# Uso, desde la carpeta del repositorio: sh zerlb/herramientas/novedades.sh

set -u
cd "$(git rev-parse --show-toplevel)" || exit 1
git remote get-url upstream >/dev/null 2>&1 \
    || git remote add upstream https://github.com/vercel-labs/agent-desktop.git
git fetch -q upstream --tags || { echo "No pude consultar vercel-labs/agent-desktop."; exit 1; }

base=$(git merge-base HEAD upstream/main)
nuevos=$(git rev-list --count "$base..upstream/main")
propios=$(git rev-list --count "upstream/main..HEAD")
echo "Original (vercel-labs): $(git log -1 --format='%h del %ad' --date=short upstream/main)"
echo "Esta copia: $(git log -1 --format='%h del %ad' --date=short HEAD) · $propios commits propios"
echo "Commits nuevos del original sin juntar: $nuevos"
if [ "$nuevos" -gt 0 ]; then
    git log --format='  %h %ad %s' --date=short "$base..upstream/main" | head -25
fi

ultima=$(git tag --list 'v*' --sort=-v:refname | head -1)
echo "Último lanzamiento del original: ${ultima:-ninguno}"
if command -v agent-desktop >/dev/null 2>&1; then
    instalada=$(agent-desktop version 2>/dev/null | grep -o '"data":{[^}]*}' | grep -o '"version":"[^"]*"' | cut -d'"' -f4)
    echo "Versión instalada en este equipo: ${instalada:-desconocida}"
fi
if [ "$nuevos" -gt 0 ]; then
    echo "Para juntarlos: git merge upstream/main, compilar, correr pruebas y el banco web."
fi
