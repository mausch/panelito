{
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-parts.url = "github:hercules-ci/flake-parts";
  };

  outputs = inputs@{ flake-parts, nixpkgs, self }:
    flake-parts.lib.mkFlake { inherit inputs; } {
      flake = {
      };
      systems = [
        "x86_64-linux"
        "aarch64-linux"
      ];
      perSystem = { options, specialArgs, lib, system, config, pkgs }: 
        let deps = [
            # for ddc-hi
            pkgs.udev
            pkgs.pkg-config
        ];
        in
        {
          devShells.default = pkgs.mkShell {
            packages = [
              pkgs.mosquitto
              pkgs.rustc
              pkgs.cargo
              
              (pkgs.vscode-with-extensions.override {
                vscode = pkgs.vscodium;
                vscodeExtensions = (with pkgs.vscode-extensions; [
                  rust-lang.rust-analyzer
                  vadimcn.vscode-lldb
                ]);
              })
            ] ++ deps;
          };

          packages = {
            default = pkgs.rustPlatform.buildRustPackage {
              name = "mqtt-light";
              src = pkgs.lib.cleanSource ./.;
              buildInputs = deps;
              nativeBuildInputs = deps;
              cargoLock = {
                lockFile = ./Cargo.lock;
                # Allow dependencies to be fetched from git and avoid having to set the outputHashes manually
                allowBuiltinFetchGit = true;
              };
              doCheck = false;
            };
          };

        };
      };
}
