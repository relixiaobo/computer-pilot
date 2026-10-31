# Source from a task-owned shell file that sets CU_BIN, CU_APP and
# COMPUTER_PILOT_CLIENT_KEY. These functions only invoke the public CLI.
# A group stops on the first failed command; cu_see observes its result once.
set -e

cu_do() {
    case "$1" in
        click|type|key)
            command "${CU_BIN:?set the verified Computer Pilot binary path}" --compact "$@" \
                --app "${CU_APP:?set the chosen pid selector}" --no-snapshot --no-verify
            ;;
        set-value|perform|scroll|hover|drag)
            command "${CU_BIN:?set the verified Computer Pilot binary path}" --compact "$@" \
                --app "${CU_APP:?set the chosen pid selector}" --no-snapshot
            ;;
        *)
            printf 'cu_do accepts action commands; use cu_see or the CLI for reads\n' >&2
            return 2
            ;;
    esac
}

cu_see() {
    command "${CU_BIN:?set the verified Computer Pilot binary path}" --compact snapshot \
        "${CU_APP:?set the chosen pid selector}" --limit 200
}
