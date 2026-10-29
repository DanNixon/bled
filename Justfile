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

check: check-nix-fmt check-typos check-effects
    @just run-sub lib/core check
    @just run-sub lib/effect check
    @just run-sub firmware check
    @just run-sub web check


build: build-effects
    @just run-sub firmware build
    @just run-sub web build

check-effects:
    @just run-sub effects/span-colour-chase check
    @just run-sub effects/span-painter check

build-effects:
    @just run-sub effects/span-colour-chase build
    @just run-sub effects/span-painter build

check-nix-fmt:
    alejandra -q -c .

check-typos:
    typos

fmt: fmt-nix

fmt-nix:
    alejandra -q .
