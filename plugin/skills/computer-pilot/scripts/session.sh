# Source from a task-owned file exporting CU_BIN, CU_APP and the task's
# COMPUTER_PILOT_* environment. Every call uses the same binary and session.
set -e

cu() {
    local cu_format=--compact
    case "$1" in
        --compact|--json|--human) cu_format="$1"; shift ;;
    esac
    case "$1" in
        --*)
            printf '%s\n' 'Use "$CU_BIN" with explicit app targeting for other root options.' >&2
            return 2
            ;;
        click|type|key)
            set -- "$@" --app "${CU_APP:?set the chosen pid selector}" --no-snapshot --no-verify
            ;;
        set-value|perform|scroll|hover|drag)
            set -- "$@" --app "${CU_APP:?set the chosen pid selector}" --no-snapshot
            ;;
        snapshot)
            if [ "$#" -eq 1 ]; then set -- "$@" --limit 200; fi
            set -- "$@" "${CU_APP:?set the chosen pid selector}"
            ;;
        state|ocr|menu)
            set -- "$@" "${CU_APP:?set the chosen pid selector}"
            ;;
        wait|find|why|screenshot|nearest|observe-region)
            set -- "$@" --app "${CU_APP:?set the chosen pid selector}"
            ;;
    esac
    command "${CU_BIN:?set the verified Computer Pilot binary path}" "$cu_format" "$@"
}
