#!/bin/sh
# Comando forçado da chave de leitura do Claude (settings/SSH/authorized_keys, command=...).
# Só leitura e só em /mnt/us. Os argumentos chegam um por linha (o `kindle` da VPS manda assim),
# sem passar por eval: nada do que vem do outro lado vira comando do shell.
# Aceita: ls, cat, head, tail, wc, stat, du, grep, find (só -name/-iname/-type/-maxdepth).

die() { echo "kindle: $*" >&2; exit 2; }

ok_path() {
    case "$1" in
        *..*) return 1 ;;
        /mnt/us | /mnt/us/*) return 0 ;;
        *) return 1 ;;
    esac
}

IFS='
'
set -f
# shellcheck disable=SC2086
set -- $SSH_ORIGINAL_COMMAND
[ $# -ge 1 ] || die "uso: kindle <ls|cat|head|tail|wc|stat|du|grep|find> ..."
verb="$1"
shift

paths_only() {
    for a in "$@"; do
        case "$a" in
            -*) ;;
            *) ok_path "$a" || die "fora de /mnt/us: $a" ;;
        esac
    done
}

case "$verb" in
    ls | cat | stat | wc)
        for a in "$@"; do
            case "$a" in
                -l | -la | -al | -lh | -a | -1 | -c | -w) ;;
                -*) die "opção não permitida: $a" ;;
                *) ok_path "$a" || die "fora de /mnt/us: $a" ;;
            esac
        done
        exec "$verb" "$@"
        ;;
    head | tail)
        n=""
        files=""
        while [ $# -gt 0 ]; do
            case "$1" in
                -n)
                    shift
                    case "$1" in '' | *[!0-9]*) die "número inválido" ;; esac
                    n="$1"
                    ;;
                -*) die "opção não permitida: $1" ;;
                *) ok_path "$1" || die "fora de /mnt/us: $1"; files="$1" ;;
            esac
            shift
        done
        [ -n "$files" ] || die "falta o arquivo"
        exec "$verb" -n "${n:-40}" "$files"
        ;;
    du)
        paths_only "$@"
        exec du -sh "$@"
        ;;
    grep)
        opts=""
        pattern=""
        set_pat=0
        for a in "$@"; do
            case "$a" in
                -i | -n | -l | -c | -r | -ri | -rn | -in | -il | -rl | -ril) opts="$opts
$a" ;;
                -*) die "opção não permitida: $a" ;;
                *)
                    if [ $set_pat = 0 ]; then
                        pattern="$a"
                        set_pat=1
                    else
                        ok_path "$a" || die "fora de /mnt/us: $a"
                    fi
                    ;;
            esac
        done
        [ $set_pat = 1 ] || die "falta o padrão"
        # Remonta: opções, padrão (com -e, para não virar opção) e caminhos.
        set -f
        args=""
        skip=1
        for a in "$@"; do
            case "$a" in -*) continue ;; esac
            if [ $skip = 1 ]; then skip=0; continue; fi
            args="$args
$a"
        done
        # shellcheck disable=SC2086
        exec grep $opts -e "$pattern" $args
        ;;
    find)
        [ $# -ge 1 ] || die "falta o caminho"
        ok_path "$1" || die "fora de /mnt/us: $1"
        root="$1"
        shift
        expr=""
        while [ $# -gt 0 ]; do
            case "$1" in
                -name | -iname)
                    [ $# -ge 2 ] || die "falta o padrão"
                    expr="$expr
$1
$2"
                    shift
                    ;;
                -type)
                    case "$2" in f | d) ;; *) die "tipo inválido" ;; esac
                    expr="$expr
-type
$2"
                    shift
                    ;;
                -maxdepth)
                    case "$2" in '' | *[!0-9]*) die "profundidade inválida" ;; esac
                    expr="-maxdepth
$2$expr"
                    shift
                    ;;
                *) die "expressão não permitida: $1" ;;
            esac
            shift
        done
        # shellcheck disable=SC2086
        exec find "$root" $expr
        ;;
    *) die "comando não permitido: $verb (use ls, cat, head, tail, wc, stat, du, grep, find)" ;;
esac
