#!/bin/sh
# Invoked by a task-owned launcher that exports CU_BIN, CU_APP and COMPUTER_PILOT_*.
# Keep CLI action defaults; callers explicitly defer intermediate observations.
set -e

cu_format=--compact
case "$1" in
    --compact|--json|--human) cu_format="$1"; shift ;;
esac
case "$1" in
    --help|-h|--version|-V) ;;
    --*)
        printf '%s\n' 'Only --compact, --json or --human may precede the command in the task launcher.' >&2
        exit 2
        ;;
    click|type|key|set-value|perform|scroll|hover|drag)
        # Keep --app before a caller's `--` text separator. Appending it after
        # that separator makes clap parse it as another positional argument,
        # so `type -- ...` fails before dispatch.
        cu_command=$1
        shift
        set -- "$cu_command" --app "${CU_APP:?set the chosen pid selector}" "$@"
        ;;
    snapshot)
        if [ "$#" -eq 1 ]; then set -- "$@" --limit 200; fi
        set -- "$@" "${CU_APP:?set the chosen pid selector}"
        ;;
    state|ocr|menu)
        set -- "$@" "${CU_APP:?set the chosen pid selector}"
        ;;
    wait|find|why|screenshot|nearest|observe-region|window)
        set -- "$@" --app "${CU_APP:?set the chosen pid selector}"
        ;;
esac
exec "${CU_BIN:?set the verified Computer Pilot binary path}" "$cu_format" "$@"
