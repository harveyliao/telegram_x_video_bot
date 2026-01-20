{
  description = "xbot - Teloxide + yt-dlp";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";

    # sops-nix (在 module 里用)
    sops-nix.url = "github:Mic92/sops-nix";
    sops-nix.inputs.nixpkgs.follows = "nixpkgs";
  };

  outputs = { self, nixpkgs, sops-nix }:
    let
      systems = [ "x86_64-linux" "aarch64-linux" ];
      forAllSystems = f: nixpkgs.lib.genAttrs systems (system: f system);
    in
    {
      packages = forAllSystems (system:
        let pkgs = import nixpkgs { inherit system; };
        in {
          xbot = pkgs.rustPlatform.buildRustPackage {
            pname = "xbot";
            version = "0.1.0";
            src = ./.;
            cargoLock.lockFile = ./Cargo.lock;

            nativeBuildInputs = [ pkgs.pkg-config ];
            buildInputs = [ pkgs.openssl ];
          };

          default = self.packages.${system}.xbot;
        }
      );

      overlays.default = final: prev: {
        xbot = self.packages.${final.system}.xbot;
      };

      nixosModules.default = import ./nix/module.nix {
        inherit sops-nix;
      };
    };
}

