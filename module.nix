self:
{
  config,
  lib,
  pkgs,
  ...
}:

let
  cfg = config.services.niri-remote;
  package = self.packages.${pkgs.system}.default;
  webPackage = self.packages.${pkgs.system}.web;
in
{
  options.services.niri-remote = {
    enable = lib.mkEnableOption "niri-remote desktop remote control";

    port = lib.mkOption {
      type = lib.types.port;
      default = 9876;
      description = "Port for the HTTP server (binds 127.0.0.1 only)";
    };
  };

  config = lib.mkIf cfg.enable {
    users.groups.uinput = { };
    users.users.max.extraGroups = [ "uinput" ];

    services.udev.extraRules = ''
      KERNEL=="uinput", SUBSYSTEM=="misc", MODE="0660", GROUP="uinput"
    '';

    systemd.user.services.niri-remote = {
      description = "niri-remote desktop remote control";
      wantedBy = [ "graphical-session.target" ];
      after = [ "graphical-session.target" ];

      environment = {
        NIRI_REMOTE_PORT = toString cfg.port;
        NIRI_REMOTE_WEB_DIR = "${webPackage}";
        NIRI_SOCKET = "%t/niri-socket";
        RUST_LOG = "info";
      };

      serviceConfig = {
        ExecStart = "${package}/bin/niri-remote-server";
        Restart = "on-failure";
        RestartSec = 5;
      };
    };
  };
}
