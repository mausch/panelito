{
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
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
        
        (pkgs.vscode-with-extensions.override {
          vscode = pkgs.vscodium;
          vscodeExtensions = (with pkgs.vscode-extensions; [
            rust-lang.rust-analyzer
            vadimcn.vscode-lldb
          ]);
        })
      ];
    };

    # packages."${system}".default = pkgs.hello;

  };
}
