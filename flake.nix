{
  description = "telemax — remote control daemon for Niri Wayland compositor";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs = { self, nixpkgs, flake-utils }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        pkgs = nixpkgs.legacyPackages.${system};

        web = pkgs.buildNpmPackage {
          pname = "telemax-web";
          version = "0.1.0";
          src = ./web;
          npmDepsHash = "sha256-8+iZKGIQgl+IAYvfC4P75SdUH+qXq4YenepMiRzrNM4=";

          env.TELEMAX_BASE_PATH = "/telemax";

          buildPhase = ''
            npm run build
          '';

          installPhase = ''
            cp -r build $out
          '';
        };

        server = pkgs.rustPlatform.buildRustPackage {
          pname = "telemax-server";
          version = "0.1.0";
          src = ./server;
          cargoHash = "sha256-f6VNnACG3G63TWI21XRBu5/P7j/BCR4wcGu2li1j5p4=";

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
