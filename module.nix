self:

{ config
, lib
, pkgs
, ...
}:

with lib;

let
  cfg = config.services.hypixel-bank-tracker;

  options = { name, ... }: {
    options = {
      enable = lib.mkEnableOption "hypixel-bank-tracker instance";

      name = mkOption {
        type = types.str;
        default = name;
      };

      package = mkPackageOption self.packages.${pkgs.system} "hypixel-bank-tracker" { };

      port = mkOption {
        type = types.port;
      };

      environmentFile = mkOption {
        type = types.nullOr types.path;
        default = null;
        example = "/etc/hypixel-bank-tracker.env";
        description = ''
          Additional environment file as defined in {manpage}`systemd.exec(5)`.

          Sensitive secrets such as {env}`HBT_HYPIXEL_API_KEY`,
          and {env}`HBT_PROFILE_UUID` may be passed to the service
          without making them world readable in the nix store.
        '';
      };
    };
  };
in
{
  options = {
    services.hypixel-bank-tracker.instances = mkOption {
      default = { };
      type = types.attrsOf (types.submodule options);
      example = { };
    };
  };

  config =
    let
      mkInstanceServiceConfig = instance: {
        description = "hypixel-bank-tracker ${instance.name} service";
        wantedBy = [ "multi-user.target" ];
        after = [ "network-online.target" ];
        wants = [ "network-online.target" ];

        serviceConfig = {
          Type = "simple";
          ExecStart = "${lib.getExe instance.package}";
          Restart = "always";
          RestartSec = "10s";
          EnvironmentFile = lib.mkIf (instance.environmentFile != null) instance.environmentFile;

          # state directory
          StateDirectory = "hypixel-bank-tracker-${instance.name}";
          WorkingDirectory = "/var/lib/hypixel-bank-tracker-${instance.name}";

          # security hardening
          NoNewPrivileges = true;
          PrivateTmp = true;
          ProtectSystem = "strict";
          ProtectHome = true;
          ReadWritePaths = [ "/var/lib/hypixel-bank-tracker-${instance.name}" ];
        };

        environment = {
          HBT_PORT = toString instance.port;
        };
      };
    in
    {
      systemd.services = lib.mkMerge (
        map
          (instance: lib.mkIf instance.enable {
            "hypixel-bank-tracker-${instance.name}" = mkInstanceServiceConfig instance;
          })
          (lib.attrValues cfg.instances)
      );
    };
}
