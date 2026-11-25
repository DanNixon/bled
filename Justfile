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


build: pcbs-fab pcbs-models dist-effects
    @just run-sub firmware build
    @just run-sub web build

check-effects:
    @just run-sub effects/matrix-colour-chase check
    @just run-sub effects/matrix-game-of-life check
    @just run-sub effects/span-colour-chase check
    @just run-sub effects/span-painter check

build-effects:
    @just run-sub effects/matrix-colour-chase build
    @just run-sub effects/matrix-game-of-life build
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

size-report:
    #!/usr/bin/env bash
    set -euo pipefail
    just build >&2
    fmt_num() {
        echo "$1" | sed ':a;s/\B[0-9]\{3\}\>/,&/;ta'
    }
    echo "### Firmware"
    echo ""
    echo "| Flavour | \`text\` | \`data\` | \`bss\` |"
    echo "| :--- | ---: | ---: | ---: |"
    for elf in firmware/target/thumbv8m.main-none-eabihf/release/bled-*; do
        [ -f "$elf" ] || continue
        case "$elf" in
            *.uf2|*.d) continue ;;
        esac
        flavour="${elf##*bled-}"
        read -r text data bss _ < <(size "$elf" | tail -n 1)
        printf "| \`%s\` | %s | %s | %s |\n" \
            "$flavour" \
            "$(fmt_num "$text")" \
            "$(fmt_num "$data")" \
            "$(fmt_num "$bss")"
    done
    echo ""
    echo "### Effect WASM Modules"
    echo ""
    echo "| Effect | Size (bytes) | Size |"
    echo "| :--- | ---: | ---: |"
    for cargo_toml in effects/*/Cargo.toml; do
        [ -f "$cargo_toml" ] || continue
        dir=$(dirname "$cargo_toml")
        effect=$(basename "$dir")
        name=$(echo "$effect" | tr '-' '_')
        wasm="$dir/target/wasm32-unknown-unknown/release/$name.wasm"
        if [ -f "$wasm" ]; then
            bytes=$(stat -c "%s" "$wasm")
            human=$(awk -v b="$bytes" 'BEGIN { printf "%.1f KiB", b / 1024 }')
            printf "| \`%s\` | %s | %s |\n" "$effect" "$(fmt_num "$bytes")" "$human"
        fi
    done

pcbs-fab:
    @just run-sub pcb/bled-badge fab
    @just run-sub pcb/bled-pico-w-backpack fab
    @just run-sub pcb/bled-stamp fab

pcbs-models:
    @just run-sub pcb/bled-badge step stl
    @just run-sub pcb/bled-pico-w-backpack step stl
    @just run-sub pcb/bled-stamp step stl
