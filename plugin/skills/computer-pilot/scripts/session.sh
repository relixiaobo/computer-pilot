# Source from a task-owned file exporting CU_BIN, CU_APP and the task's
# COMPUTER_PILOT_* environment. Every call uses the same binary and session.
set -e

cu() {
    case "$1" in
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
    # Root options are an explicit ordinary-CLI invocation (e.g. --json).
    case "$1" in --*) ;; *) set -- --compact "$@" ;; esac
    command "${CU_BIN:?set the verified Computer Pilot binary path}" "$@"
}
