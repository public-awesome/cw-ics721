#!/bin/bash
# ----------------------------------------------------
# - exports CHAIN based on user input  -
# ----------------------------------------------------

function select_chain() {
    # Read chains from the file into an array
    CHAIN_CHOICES=("stargaze" "osmosis")
    echo "Available chains: ${CHAIN_CHOICES[*]}" >&2

    echo "Please select the chain:" >&2
    select SELECTED_CHAIN in "${CHAIN_CHOICES[@]}" "Exit"; do
        case $SELECTED_CHAIN in
            "Exit") echo "Exiting..." >&2; return 0 ;;
            *) if [[ " ${CHAIN_CHOICES[*]} " =~ " ${SELECTED_CHAIN} " ]]; then
                    echo "Selected chain: $SELECTED_CHAIN" >&2
                    export CHAIN="$SELECTED_CHAIN"
                    break
            else
                    echo "Invalid choice. Please try again." >&2
            fi ;;
        esac
    done

    export CHAIN="$SELECTED_CHAIN"
    echo $CHAIN
}

export -f select_chain