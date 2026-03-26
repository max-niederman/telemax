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

        host = pkgs.rustPlatform.buildRustPackage {
          pname = "telemax-host";
          version = "0.1.0";
          src = ./host;
          cargoHash = "sha256-JQ/AMBaUtJ+JvvWgbPMun7oDzs2MMDS+/4Vi6L6r9VQ=";
        };

        # Native messaging manifest for the browser extension
        native-messaging-host = pkgs.writeTextDir
          "lib/mozilla/native-messaging-hosts/org.kde.plasma.browser_integration.json"
          (builtins.toJSON {
            name = "org.kde.plasma.browser_integration";
            description = "Telemax MPRIS bridge for browser media control";
            path = "${host}/bin/telemax-host";
            type = "stdio";
            allowed_extensions = [ "nickel-browser-integration@nickel.nickel" "plasma-browser-integration@kde.org" ];
          });
      in
      {
        packages = {
          default = server;
          inherit server web host native-messaging-host;
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
