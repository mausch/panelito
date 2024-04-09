{
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-parts.url = "github:hercules-ci/flake-parts";
    nixos-generators.url = "github:nix-community/nixos-generators";
    crane = {
      url = "github:ipetkov/crane";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs = inputs@{ nixos-generators, flake-parts, nixpkgs, crane, self }:
    let 
      deps = pkgs: [
          # for ddc-hi
          pkgs.udev
          pkgs.pkg-config
      ];

      build-panelito = system: pkgs: pkgs.rustPlatform.buildRustPackage {
        name = "panelito";
        src = crane.lib.${system}.cleanCargoSource ./.;
        buildInputs = deps pkgs;
        nativeBuildInputs = deps pkgs;
        cargoLock = {
          lockFile = ./Cargo.lock;
          # Allow dependencies to be fetched from git and avoid having to set the outputHashes manually
          allowBuiltinFetchGit = true;
        };
        doCheck = false;
      };
    in
    flake-parts.lib.mkFlake { inherit inputs; } {
      flake = {
      };
      systems = [
        "x86_64-linux"
        "aarch64-linux"
      ];
      perSystem = { options, specialArgs, lib, system, config, pkgs }: 
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
            ] ++ (deps pkgs);
          };

          packages = { 
            default = build-panelito system pkgs;

            rpi3-sdcard = 
              nixos-generators.nixosGenerate {
                system = "aarch64-linux";
                format = "sd-aarch64";
                specialArgs = {
                  pkgs = nixpkgs.legacyPackages."aarch64-linux" // {
                    panelito = build-panelito "aarch64-linux" nixpkgs.legacyPackages."aarch64-linux";
                  };
                };
                modules = [
                  ./rpi3.nix
                ];
              };
          };

        };
      };
}
