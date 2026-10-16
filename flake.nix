{
  inputs.nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";

  outputs = {nixpkgs, ...}: let
    system = "x86_64-linux";
    pkgs = import nixpkgs {inherit system;};
  in {
    devShells.${system}.default = pkgs.mkShell {
      packages = with pkgs; [
        # Generic tools
        just

        # Code formatting tools
        alejandra
        typos

        # Rust toolchain
        rustup
        flip-link
        probe-rs-tools
        elf2uf2-rs
        wasm-pack
        binaryen

        # PCB tools
        kikit
      ];

      LIBCLANG_PATH = "${pkgs.llvmPackages.libclang.lib}/lib";
    };
  };
}
