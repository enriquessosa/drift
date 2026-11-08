{
  description = "Drift local Linux auditor development shell";
  inputs.nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
  outputs = { self, nixpkgs }: let
    systems = [ "x86_64-linux" "aarch64-linux" ];
  in {
    packages = nixpkgs.lib.genAttrs systems (system: let
      pkgs = import nixpkgs { inherit system; };
    in {
      default = pkgs.rustPlatform.buildRustPackage {
        pname = "drift";
        version = "0.1.0";
        src = pkgs.lib.cleanSourceWith {
          src = ./.;
          filter = path: type:
            pkgs.lib.cleanSourceFilter path type
            && !(builtins.elem (baseNameOf path) [ "target" "result" ]);
        };
        cargoLock.lockFile = ./Cargo.lock;
        strictDeps = true;
        meta.mainProgram = "drift";
      };
    });
    devShells = nixpkgs.lib.genAttrs systems (system: let
      pkgs = import nixpkgs { inherit system; };
    in {
      default = pkgs.mkShell {
        packages = with pkgs; [ cargo rustc rustfmt clippy gcc pkg-config ];
      };
    });
  };
}
