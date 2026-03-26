self:
{
  config,
  lib,
  pkgs,
  ...
}:

let
  cfg = config.services.telemax;
in
{
  options.services.telemax = {
    enable = lib.mkEnableOption "telemax desktop remote control";

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

    # Tailscale Serve on sub-path /telemax
    systemd.services.telemax-serve = {
      description = "Tailscale Serve for telemax";
      wantedBy = [ "multi-user.target" ];
      after = [ "tailscaled.service" ];
      wants = [ "tailscaled.service" ];

      serviceConfig = {
        Type = "oneshot";
        RemainAfterExit = true;
        ExecStart = "${pkgs.tailscale}/bin/tailscale serve --set-path /telemax --bg ${toString cfg.port}";
        ExecStop = "${pkgs.tailscale}/bin/tailscale serve --set-path /telemax off";
      };
    };

    systemd.user.services.telemax = {
      description = "telemax desktop remote control";
      wantedBy = [ "graphical-session.target" ];
      after = [ "graphical-session.target" ];

      environment = {
        TELEMAX_PORT = toString cfg.port;
        TELEMAX_WEB_DIR = "${pkgs.telemax-web}";
        NIRI_SOCKET = "%t/niri-socket";
        RUST_LOG = "info";
      };

      serviceConfig = {
        ExecStart = "${pkgs.telemax}/bin/telemax";
        Restart = "on-failure";
        RestartSec = 5;
      };
    };
  };
}
