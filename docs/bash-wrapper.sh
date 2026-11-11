# Give path-valued options precedence, preserve spaces, and honor --.
_dohsub() {
    local cur="${COMP_WORDS[COMP_CWORD]}" prev="${COMP_WORDS[COMP_CWORD-1]}" i item
    COMPREPLY=()
    for ((i=1; i<COMP_CWORD; i++)); do
        item="${COMP_WORDS[i]}"
        case "$item" in
            --) return 0;;
            --file|--import|--doh|--domain|--port|--interface) ((i++));;
        esac
    done
    case "$prev" in
        --file|--import)
            while IFS= read -r item; do COMPREPLY+=("$item"); done < <(compgen -f -- "$cur")
            if [[ "${BASH_VERSINFO[0]}" -ge 4 ]]; then compopt -o filenames 2>/dev/null || :; fi
            return 0;;
        --doh|--domain|--port|--interface) return 0;;
    esac
    case "$cur" in
        --file=*|--import=*)
            local prefix="${cur%%=*}=" value="${cur#*=}"
            while IFS= read -r item; do COMPREPLY+=("$prefix$item"); done < <(compgen -f -- "$value")
            return 0;;
        --doh=*|--domain=*|--port=*|--interface=*) return 0;;
    esac
    _dohsub_generated "$1" "$cur" "$prev"
}

