#!/usr/bin/env bash
set -euo pipefail

PREFIX="${ORQ_PREFIX:-$HOME/.local}"
BINDIR="${ORQ_BINDIR:-$PREFIX/bin}"
REPO_URL="${ORQ_REPO_URL:-https://github.com/terracenter/agent-orchestrator.git}"
REF="${ORQ_REF:-main}"
DRY_RUN=0
YES=0

usage() {
  cat <<'USAGE'
Usage: install.sh [--dry-run] [--yes] [--prefix PATH] [--ref REF]

Instala orq en ~/.local/bin por defecto. Seguro para curl | bash:
  curl -fsSL https://raw.githubusercontent.com/terracenter/agent-orchestrator/main/scripts/install.sh | bash -s -- --dry-run

Opciones:
  --dry-run      muestra acciones sin modificar archivos
  --yes, -y      no preguntar confirmacion interactiva
  --prefix PATH  prefijo de instalacion (default: ~/.local)
  --ref REF      rama/tag/commit a instalar (default: main)
USAGE
}

log() { printf 'orq-install: %s\n' "$*"; }
fail() { printf 'orq-install: ERROR: %s\n' "$*" >&2; exit 1; }
run() {
  if [ "$DRY_RUN" -eq 1 ]; then
    printf 'orq-install: dry-run: %s\n' "$*"
  else
    "$@"
  fi
}

while [ "$#" -gt 0 ]; do
  case "$1" in
    --dry-run) DRY_RUN=1 ;;
    --yes|-y) YES=1 ;;
    --prefix) shift; PREFIX="${1:-}"; BINDIR="$PREFIX/bin" ;;
    --ref) shift; REF="${1:-}" ;;
    --help|-h) usage; exit 0 ;;
    *) fail "argumento desconocido: $1" ;;
  esac
  shift
done

[ -n "$PREFIX" ] || fail "--prefix vacio"
[ -n "$REF" ] || fail "--ref vacio"

need() { command -v "$1" >/dev/null 2>&1 || fail "falta '$1'. Instalarlo primero y reintentar."; }
need git
need go

if ! command -v rtk >/dev/null 2>&1; then
  cat >&2 <<'RTK'
orq-install: ADVERTENCIA: falta 'rtk'.
rtk es requerido por el flujo operativo de Orq para comandos y auditoria.
Opciones:
  1) Instalar rtk en ~/.local/bin.
  2) Agregar rtk existente al PATH.
  3) Continuar instalando orq, pero 'orq doctor' marcara el entorno blocked hasta resolverlo.
RTK
fi

log "repo=$REPO_URL ref=$REF bindir=$BINDIR dry_run=$DRY_RUN"
if [ "$YES" -ne 1 ] && [ "$DRY_RUN" -ne 1 ]; then
  printf 'Continuar instalacion? [y/N] '
  read -r answer
  case "$answer" in y|Y|yes|YES) ;; *) fail "cancelado por usuario" ;; esac
fi

tmpdir="$(mktemp -d 2>/dev/null || mktemp -d -t orq-install)"
cleanup() { rm -rf "$tmpdir"; }
trap cleanup EXIT INT TERM

if [ "$DRY_RUN" -eq 1 ]; then
  log "dry-run: se clonaria $REPO_URL en $tmpdir/src y se instalaria orq en $BINDIR/orq"
  exit 0
fi

git clone --depth 1 --branch "$REF" "$REPO_URL" "$tmpdir/src"
cd "$tmpdir/src"
go build -buildvcs=false -o "$tmpdir/orq" ./cmd/orq
mkdir -p "$BINDIR"
if [ -e "$BINDIR/orq" ]; then
  backup="$BINDIR/orq.backup.$(date -u +%Y%m%dT%H%M%SZ)"
  log "backup $BINDIR/orq -> $backup"
  cp -p "$BINDIR/orq" "$backup"
fi
install -m 0755 "$tmpdir/orq" "$BINDIR/orq"
log "instalado $BINDIR/orq"
log "ejecuta: $BINDIR/orq doctor"
