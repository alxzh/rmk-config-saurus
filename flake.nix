{
  description = "RMK firmware development shell for the Saurus keyboard";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    rust-overlay.url = "github:oxalica/rust-overlay";
    rust-overlay.inputs.nixpkgs.follows = "nixpkgs";
  };

  outputs = { nixpkgs, rust-overlay, ... }:
    let
      systems = [
        "aarch64-darwin"
        "x86_64-darwin"
        "aarch64-linux"
        "x86_64-linux"
      ];
      forAllSystems = nixpkgs.lib.genAttrs systems;
    in
    {
      devShells = forAllSystems (system:
        let
          pkgs = import nixpkgs {
            inherit system;
            overlays = [ rust-overlay.overlays.default ];
          };

          rustToolchain = pkgs.rust-bin.fromRustupToolchainFile ./rust-toolchain.toml;

          cargo-hex-to-uf2 = pkgs.rustPlatform.buildRustPackage {
            pname = "cargo-hex-to-uf2";
            version = "0.1.2";

            src = pkgs.fetchCrate {
              pname = "cargo-hex-to-uf2";
              version = "0.1.2";
              hash = "sha256-5s7tn/rlZPeB+JjumX+NYOCCaY0Q2UoQQAyj47OcLAU=";
            };

            cargoHash = "sha256-4bNL5W0OF8NbRTFhDrArdpjRsSWoTwnnTDcPcOvPSNU=";
            doCheck = false;
          };
        in
        {
          default = pkgs.mkShell {
            packages = [
              rustToolchain
              pkgs.cargo-binutils
              cargo-hex-to-uf2
              pkgs.cargo-make
              pkgs.flip-link
              pkgs.probe-rs-tools
            ];

            DEFMT_LOG = "debug";
            RUST_SRC_PATH = "${rustToolchain}/lib/rustlib/src/rust/library";

            shellHook = ''
              echo "RMK dev shell ready: run 'cargo make uf2 --release'"

              if [[ $- == *i* && -z "''${RMK_NIX_DEVELOP_NO_SHELL_HANDOFF:-}" ]]; then
                preferred_shell="''${SHELL:-}"

                if [[ -z "$preferred_shell" || ! -x "$preferred_shell" || "$preferred_shell" == "''${BASH:-}" ]]; then
                  preferred_shell=""

                  if command -v getent >/dev/null 2>&1 && [[ -n "''${USER:-}" ]]; then
                    preferred_shell="$(getent passwd "''${USER}" | cut -d: -f7)"
                  elif command -v dscl >/dev/null 2>&1 && [[ -n "''${USER:-}" ]]; then
                    preferred_shell="$(dscl . -read "/Users/''${USER}" UserShell 2>/dev/null | awk '{print $2}')"
                  fi
                fi

                if [[ -z "$preferred_shell" || ! -x "$preferred_shell" || "$preferred_shell" == "''${BASH:-}" ]]; then
                  if command -v fish >/dev/null 2>&1; then
                    preferred_shell="$(command -v fish)"
                  fi
                fi

                if [[ -n "$preferred_shell" && -x "$preferred_shell" && "$preferred_shell" != "''${BASH:-}" ]]; then
                  export SHELL="$preferred_shell"
                  exec "$preferred_shell"
                fi
              fi
            '';
          };
        });
    };
}
