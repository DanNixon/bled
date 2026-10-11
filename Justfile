default: check

[private]
run-sub dir *recipe="":
    @if [ -n "${GITHUB_ACTIONS:-}" ]; then \
        echo "::group::{{dir}} {{recipe}}"; \
    elif [ -n "{{recipe}}" ]; then \
        printf "\n\033[1;36m━━━ [%s] just %s ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\033[0m\n" "{{dir}}" "{{recipe}}"; \
    else \
        printf "\n\033[1;36m━━━ [%s] ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\033[0m\n" "{{dir}}"; \
    fi
    @just -f {{dir}}/Justfile {{recipe}}
    @if [ -n "${GITHUB_ACTIONS:-}" ]; then echo "::endgroup::"; fi

check: check-nix-fmt check-typos
    @just run-sub core check
    @just run-sub firmware check
    @just run-sub web check

build:
    @just run-sub firmware build
    @just run-sub web build

check-nix-fmt:
    alejandra -q -c .

check-typos:
    typos

fmt: fmt-nix

fmt-nix:
    alejandra -q .
