{
  description = "niri-remote — remote control daemon for Niri Wayland compositor";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs = { self, nixpkgs, flake-utils }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        pkgs = nixpkgs.legacyPackages.${system};

        web = pkgs.buildNpmPackage {
          pname = "niri-remote-web";
          version = "0.1.0";
          src = ./web;
          npmDepsHash = pkgs.lib.fakeHash;

          buildPhase = ''
            npm run build
          '';

          installPhase = ''
            cp -r build $out
          '';
        };

        server = pkgs.rustPlatform.buildRustPackage {
          pname = "niri-remote-server";
          version = "0.1.0";
          src = ./server;
          cargoHash = pkgs.lib.fakeHash;

          nativeBuildInputs = [ pkgs.pkg-config ];
          buildInputs = [ pkgs.libpulseaudio ];
        };
      in
      {
        packages = {
          default = server;
          inherit server web;
        };

        devShells.default = pkgs.mkShell {
          nativeBuildInputs = with pkgs; [
            cargo
            rustc
            rust-analyzer
            clippy
            pkg-config
            nodejs
          ];

          buildInputs = with pkgs; [
            libpulseaudio
          ];
        };
      }
    ) // {
      nixosModules.default = import ./module.nix self;
    };
}
