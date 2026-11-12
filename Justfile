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
    @just run-sub effects/matrix-colour-chase check
    @just run-sub effects/span-colour-chase check
    @just run-sub effects/span-painter check

build-effects:
    @just run-sub effects/matrix-colour-chase build
    @just run-sub effects/span-colour-chase build
    @just run-sub effects/span-painter build

dist-effects: build-effects
    @rm -rf effects/dist
    @for cargo_toml in effects/*/Cargo.toml; do \
        [ -f "$cargo_toml" ] || continue; \
        dir=$(dirname "$cargo_toml"); \
        name=$(basename "$dir"); \
        name=$(echo "$name" | tr '-' '_'); \
        if [ -f "$dir/target/wasm32-unknown-unknown/release/$name.wasm" ]; then \
            wasm="$dir/target/wasm32-unknown-unknown/release/$name.wasm"; \
        else \
            wasm=""; \
        fi; \
        if [ -n "$wasm" ]; then \
            mkdir -p "effects/dist/$name"; \
            cp "$wasm" "effects/dist/$name/$name.wasm"; \
        fi; \
        presets_dir="$dir/target/wasm32-unknown-unknown/release/presets"; \
        if [ -d "$presets_dir" ]; then \
            for preset in "$presets_dir"/*.json; do \
                if [ -f "$preset" ]; then \
                    mkdir -p "effects/dist/$name/presets"; \
                    cp "$preset" "effects/dist/$name/presets/"; \
                fi; \
            done; \
        fi; \
    done

check-nix-fmt:
    alejandra -q -c .

check-typos:
    typos

fmt: fmt-nix

fmt-nix:
    alejandra -q .
