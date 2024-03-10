{
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    # ddc-hi.url = "github:arcnmx/ddc-hi-rs";
  };

  outputs = { self, nixpkgs }: 
    let 
      system = "x86_64-linux";
      pkgs = nixpkgs.legacyPackages."${system}";
    in
  {

    devShells.${system}.default = pkgs.mkShell {
      packages = [
        pkgs.mosquitto
        pkgs.rustc
        pkgs.cargo

        # for ddc-hi
        pkgs.udev
        pkgs.pkg-config
        # ddc-hi.lib.crate
        
        (pkgs.vscode-with-extensions.override {
          vscode = pkgs.vscodium;
          vscodeExtensions = (with pkgs.vscode-extensions; [
            rust-lang.rust-analyzer
            vadimcn.vscode-lldb
          ]);
        })
      ];
    };

    packages."${system}".default =
      pkgs.rustPlatform.buildRustPackage {
        name = "mqtt-light";
        src = pkgs.lib.cleanSource ./.;
        cargoLock = {
          lockFile = ./Cargo.lock;
          # Allow dependencies to be fetched from git and avoid having to set the outputHashes manually
          allowBuiltinFetchGit = true;
        };
        doCheck = false;
      };

  };
}
