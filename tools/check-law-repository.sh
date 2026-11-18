#!/bin/sh
# Check every workspace at the top of a directory with a release build of fidryn.
#
# Usage: tools/check-law-repository.sh [--cache-dir CACHE] DIR
#
# A workspace at the top of DIR is a directory directly in DIR that holds
# workspace.json. For each, the script compares the version of the toolchain
# its manifest records with the version of the binary, and runs
# `fidryn check` on it. It exits 0 only when every version is the binary's
# and every check exits 0; a warning, such as a stale binding, does not fail
# a check. It says for each workspace what it found.
#
# The binary is the one FIDRYN_BINARY names; without it, the script builds
# the release binary with `cargo build --release -p fidryn-cli --locked`,
# through `lease run` when there is a command lease, and directly when there
# is none, and fails when the build fails.
#
# Each run of the binary is limited to FIDRYN_CHECK_TIMEOUT seconds of time
# (600 by default) with timeout, when there is such a command, and to
# FIDRYN_CHECK_CPU seconds of processor time (900) and
# FIDRYN_CHECK_ADDRESS_SPACE bytes of address space (8589934592) with
# prlimit, when there is such a command. Where one of the two is missing,
# the script says once which limits it does not apply, and checks all the
# same.
#
# The cache of the checks is CACHE, else FIDRYN_CACHE_DIR, else
# fidryn/law-repository in XDG_CACHE_HOME or in ~/.cache; it may be neither
# DIR, nor inside it, nor hold it, so that nothing is written into DIR. It
# is made only when DIR holds a workspace.
#
# Exit status: 0 when everything passed, 1 when a version differs or a check
# failed, 2 when the script could not check, such as on a usage error, a
# failed build, or a DIR with no workspace at its top.

set -eu
LC_ALL=C
export LC_ALL
# A directory the script names is the one it names, whatever CDPATH says.
unset CDPATH

name=check-law-repository
usage="usage: tools/check-law-repository.sh [--cache-dir CACHE] DIR"

say() {
    printf '%s\n' "$*"
}

refuse() {
    printf '%s: %s\n' "$name" "$*" >&2
    exit 2
}

# The value of the variable named $1, or $2 when it is not set, which must be
# a whole number greater than 0.
limit() {
    value=
    eval "value=\${$1-$2}"
    case $value in
        '' | *[!0-9]* | 0*) refuse "$1 is \"$value\", and a limit is a whole number greater than 0" ;;
    esac
    printf '%s\n' "$value"
}

# $1 made absolute, with every link and `..` resolved in the part of it that
# exists; the rest is taken as written.
resolved() {
    case $1 in
        /*) at=$1 ;;
        *) at=$(pwd -P)/$1 ;;
    esac
    while [ "$at" != / ] && [ "${at%/}" != "$at" ]; do
        at=${at%/}
    done
    rest=
    while [ ! -d "$at" ]; do
        rest=/${at##*/}$rest
        at=${at%/*}
        [ -n "$at" ] || at=/
    done
    at=$(cd "$at" && pwd -P)
    if [ "$at" = / ]; then
        printf '%s\n' "${rest:-/}"
    else
        printf '%s%s\n' "$at" "$rest"
    fi
}

# Whether the directories $1 and $2, both absolute, are one, or one lies
# inside the other.
overlap() {
    case "$1/" in "$2"/*) return 0 ;; esac
    case "$2/" in "$1"/*) return 0 ;; esac
    return 1
}

cache=
cache_from=
while [ $# -gt 0 ]; do
    case $1 in
        --cache-dir)
            [ $# -ge 2 ] || refuse "$usage"
            cache=$2
            cache_from="from --cache-dir"
            shift 2
            ;;
        -h | --help)
            sed -n '2,/^$/s/^#[ ]\{0,1\}//p' "$0"
            exit 0
            ;;
        --)
            shift
            break
            ;;
        -*) refuse "unknown option $1; $usage" ;;
        *) break ;;
    esac
done
[ $# -eq 1 ] || refuse "$usage"
[ -d "$1" ] || refuse "$1 is not a directory"
dir=$(cd "$1" && pwd -P)

time_limit=$(limit FIDRYN_CHECK_TIMEOUT 600)
cpu_limit=$(limit FIDRYN_CHECK_CPU 900)
space_limit=$(limit FIDRYN_CHECK_ADDRESS_SPACE 8589934592)
has_timeout=no
has_prlimit=no
if command -v timeout >/dev/null 2>&1; then has_timeout=yes; fi
if command -v prlimit >/dev/null 2>&1; then has_prlimit=yes; fi

# Runs the binary with the limits there are commands for.
run() {
    case $has_timeout$has_prlimit in
        yesyes) timeout "$time_limit" prlimit --as="$space_limit" --cpu="$cpu_limit" -- "$binary" "$@" ;;
        yesno) timeout "$time_limit" "$binary" "$@" ;;
        noyes) prlimit --as="$space_limit" --cpu="$cpu_limit" -- "$binary" "$@" ;;
        *) "$binary" "$@" ;;
    esac
}

workspaces=
for entry in "$dir"/*; do
    if [ -d "$entry" ] && [ -f "$entry/workspace.json" ]; then
        workspaces="$workspaces${workspaces:+
}${entry##*/}"
    fi
done
if [ -z "$workspaces" ]; then
    if [ -f "$dir/workspace.json" ]; then
        refuse "$dir holds no workspace at its top; it is a workspace itself, so name the directory that holds it"
    fi
    refuse "$dir holds no workspace at its top: no directory directly in it holds workspace.json"
fi

if [ -z "$cache" ] && [ -n "${FIDRYN_CACHE_DIR:-}" ]; then
    cache=$FIDRYN_CACHE_DIR
    cache_from="from FIDRYN_CACHE_DIR"
fi
if [ -z "$cache" ]; then
    home=${XDG_CACHE_HOME:-${HOME:?"set HOME, or name the cache with --cache-dir"}/.cache}
    cache=$home/fidryn/law-repository
    cache_from="the default; --cache-dir or FIDRYN_CACHE_DIR names another"
fi
cache=$(resolved "$cache")
if overlap "$cache" "$dir"; then
    refuse "the cache $cache is $dir, lies inside it or holds it; name a cache outside $dir"
fi
mkdir -p "$cache" || refuse "the cache $cache cannot be made"
cache=$(cd "$cache" && pwd -P)
if overlap "$cache" "$dir"; then
    refuse "the cache $cache is $dir, lies inside it or holds it; name a cache outside $dir"
fi

if [ -n "${FIDRYN_BINARY:-}" ]; then
    binary=$FIDRYN_BINARY
    made="given by FIDRYN_BINARY"
else
    root=$(cd "$(dirname "$0")/.." && pwd -P)
    if command -v lease >/dev/null 2>&1; then
        say "building the release binary through lease: lease run --cpus 16 --mem 25G --time 1h -- cargo build --release -p fidryn-cli --locked"
        status=0
        (cd "$root" && lease run --cpus 16 --mem 25G --time 1h -- cargo build --release -p fidryn-cli --locked) || status=$?
        [ "$status" -eq 0 ] ||
            refuse "the build through lease failed (exit $status); the script does not build without it"
    else
        say "building the release binary directly, since there is no command lease: cargo build --release -p fidryn-cli --locked"
        status=0
        (cd "$root" && cargo build --release -p fidryn-cli --locked) || status=$?
        [ "$status" -eq 0 ] || refuse "the build failed (exit $status)"
    fi
    target=${CARGO_TARGET_DIR:-target}
    case $target in
        /*) ;;
        *) target=$root/$target ;;
    esac
    binary=$target/release/fidryn
    made="built"
fi
if [ ! -f "$binary" ] || [ ! -x "$binary" ]; then
    refuse "$binary is no executable file"
fi

say "limits of each run of the binary: $time_limit s of time, $cpu_limit s of processor time, $space_limit bytes of address space"
if [ "$has_timeout" = no ]; then
    say "there is no command timeout, so the limit of $time_limit s of time is not applied"
fi
if [ "$has_prlimit" = no ]; then
    say "there is no command prlimit, so the limits of $cpu_limit s of processor time and $space_limit bytes of address space are not applied"
fi
status=0
shown=$(run --version) || status=$?
version=${shown#fidryn }
if [ "$status" -ne 0 ] || [ "$version" = "$shown" ] || [ -z "$version" ]; then
    refuse "the version of $binary cannot be read: it printed \"$shown\" and exited $status"
fi
say "binary: $binary, fidryn $version ($made)"
say "cache: $cache ($cache_from)"

checked=0
failed=
newline='
'
old_ifs=$IFS
IFS=$newline
set -f
for workspace in $workspaces; do
    IFS=$old_ifs
    checked=$((checked + 1))
    fine=yes
    say ""
    recorded=$(sed -n 's/.*"toolchain"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/p' "$dir/$workspace/workspace.json")
    case $recorded in
        "")
            say "$workspace: the manifest records no toolchain"
            fine=no
            ;;
        *"$newline"*)
            say "$workspace: the manifest records more than one toolchain"
            fine=no
            ;;
        "$version")
            say "$workspace: the manifest records the toolchain $recorded, which is the version of the binary"
            ;;
        *)
            say "$workspace: the manifest records the toolchain $recorded, and the binary is $version"
            fine=no
            ;;
    esac
    say "$workspace: fidryn check $dir/$workspace --cache-dir $cache"
    status=0
    found=$(run check "$dir/$workspace" --cache-dir "$cache") || status=$?
    [ -z "$found" ] || say "$found"
    case $status in
        0)
            case $found in
                *"warnings: 0"*) say "$workspace: the check passed" ;;
                *) say "$workspace: the check passed with warnings, which do not fail it" ;;
            esac
            ;;
        124)
            say "$workspace: the check was stopped at the limit of $time_limit s of time"
            fine=no
            ;;
        152)
            say "$workspace: the check was stopped at the limit of $cpu_limit s of processor time"
            fine=no
            ;;
        *)
            say "$workspace: the check failed (exit $status)"
            fine=no
            ;;
    esac
    [ "$fine" = yes ] || failed="$failed $workspace"
    IFS=$newline
done
set +f
IFS=$old_ifs

say ""
if [ "$checked" -eq 1 ]; then counted="1 workspace"; else counted="$checked workspaces"; fi
if [ -n "$failed" ]; then
    say "checked $counted; failed:$failed"
    exit 1
fi
say "checked $counted: every version is the binary's, and every check passed"
