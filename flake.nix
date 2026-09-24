{
  description = "Language server for niri KDL configuration files";
  inputs.nixpkgs.url = "github:NixOS/nixpkgs/nixos-25.05";
  outputs =
    { self, nixpkgs }:
    let
      eachSystem = nixpkgs.lib.genAttrs [
        "x86_64-linux"
        "aarch64-linux"
      ];
    in
    {
      packages = eachSystem (
        system:
        let
          pkgs = import nixpkgs { inherit system; };
        in
        {
          default = pkgs.rustPlatform.buildRustPackage {
            pname = "niri-lsp";
            version = "0.1.0";
            src = self;
            cargoLock.lockFile = ./Cargo.lock;
            cargoBuildFlags = [ "-p" "niri-lsp" ];
            cargoTestFlags = [ "-p" "niri-lsp" ];
          };
          xtask = pkgs.rustPlatform.buildRustPackage {
            pname = "niri-lsp-xtask";
            version = "0.1.0";
            src = self;
            cargoLock.lockFile = ./Cargo.lock;
            cargoBuildFlags = [ "-p" "xtask" ];
            cargoTestFlags = [ "-p" "xtask" ];
          };
        }
      );
      apps = eachSystem (system: {
        default = {
          type = "app";
          program = "${self.packages.${system}.default}/bin/niri-lsp";
        };
      });
      devShells = eachSystem (
        system:
        let
          pkgs = import nixpkgs { inherit system; };
        in
        {
          default = pkgs.mkShell {
            packages = with pkgs; [
              cargo
              rustc
              rustfmt
              clippy
              pkg-config
            ];
          };
        }
      );
      checks = eachSystem (
        system:
        let
          pkgs = import nixpkgs { inherit system; };
        in
        {
          build = self.packages.${system}.default;
          schema =
            pkgs.runCommand "niri-lsp-schema"
              {
                nativeBuildInputs = [ self.packages.${system}.xtask ];
              }
              ''
                cd ${self}
                ${self.packages.${system}.xtask}/bin/xtask schema check
                touch $out
              '';
          rust = pkgs.rustPlatform.buildRustPackage {
            pname = "niri-lsp-rust-checks";
            version = "0.1.0";
            src = self;
            cargoLock.lockFile = ./Cargo.lock;
            nativeBuildInputs = [ pkgs.rustfmt pkgs.clippy ];
            doCheck = true;
            checkPhase = ''
              runHook preCheck
              cargo fmt --check
              cargo test --workspace
              cargo clippy --workspace -- -D warnings
              runHook postCheck
            '';
          };
        }
      );
    };
}
