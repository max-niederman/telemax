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
  };

  config = lib.mkIf cfg.enable {
    hardware.uinput.enable = true;

    # Tailscale Serve proxying to the user's Unix socket
    systemd.services.telemax-serve = {
      description = "Tailscale Serve for telemax";
      wantedBy = [ "multi-user.target" ];
      after = [ "tailscaled.service" ];
      wants = [ "tailscaled.service" ];

      serviceConfig = {
        Type = "oneshot";
        RemainAfterExit = true;
        ExecStart = "${pkgs.tailscale}/bin/tailscale serve --set-path /telemax --bg unix:/run/user/1000/telemax.sock";
        ExecStop = "${pkgs.tailscale}/bin/tailscale serve --set-path /telemax off";
      };
    };

    systemd.user.services.telemax = {
      description = "telemax desktop remote control";
      wantedBy = [ "graphical-session.target" ];
      after = [ "graphical-session.target" ];

      environment = {
        TELEMAX_WEB_DIR = "${pkgs.telemax-web}";
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
