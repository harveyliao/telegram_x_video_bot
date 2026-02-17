{ sops-nix }:
{ config, pkgs, lib, ... }:

let
  cfg = config.services.xbot;
  videoDir = "/var/lib/xbot/videos";
  allowedUserIdsCsv = lib.concatStringsSep "," (map toString cfg.allowedUserIds);
in
{
  options.services.xbot.allowedUserIds = lib.mkOption {
    type = lib.types.listOf lib.types.int;
    default = [];
    example = [ 123456789 987654321 ];
    description = "Non-secret Telegram user IDs that are allowed to use the bot.";
  };

  imports = [
    sops-nix.nixosModules.sops
  ];

  config = {
    assertions = [
      {
        assertion = cfg.allowedUserIds != [];
        message = "services.xbot.allowedUserIds must contain at least one Telegram user ID.";
      }
    ];

    users.users.xbot = {
      isSystemUser = true;
      group = "xbot";
    };
    users.groups.xbot = {};

    # 你可以在 host 端覆盖这个路径
    # 默认假设 secrets 在 bot repo 里（更推荐 host 单独放 secrets，见下文）
    sops.defaultSopsFile = lib.mkDefault ../secrets/xbot.yaml;

    sops.secrets = {
      "xbot/token" = {
        key = "teloxide_token";
        owner = "xbot";
        group = "xbot";
        mode = "0400";
      };

      "xbot/cookie" = {
        key = "x_cookie_txt";
        owner = "xbot";
        group = "xbot";
        mode = "0400";
      };
    };

    # 用 template 生成 EnvironmentFile 给 systemd
    sops.templates."xbot.env" = {
      owner = "xbot";
      group = "xbot";
      mode = "0400";
      content = ''
        TELOXIDE_TOKEN=${config.sops.placeholder."xbot/token"}
        COOKIE_FILE=${config.sops.secrets."xbot/cookie".path}
        VIDEO_DIR=${videoDir}
        ALLOWED_USER_IDS=${allowedUserIdsCsv}
        MAX_CONCURRENT=2
        TIMEOUT_SECS=600
      '';
    };

    systemd.tmpfiles.rules = [
      "d ${videoDir} 0700 xbot xbot - -"
    ];

    systemd.services.xbot = {
      description = "X.com video downloader Telegram bot";
      wantedBy = [ "multi-user.target" ];
      after = [ "network-online.target" ];
      wants = [ "network-online.target" ];

      # 提供 yt-dlp / ffmpeg
      path = [ pkgs.yt-dlp pkgs.ffmpeg ];

      serviceConfig = {
        User = "xbot";
        Group = "xbot";

        EnvironmentFile = config.sops.templates."xbot.env".path;

        ExecStart = "${pkgs.xbot}/bin/telegram_x_video_bot";

        Restart = "on-failure";
        RestartSec = 2;

        StateDirectory = "xbot";
        CacheDirectory = "xbot";

        # 关键：给 yt-dlp 一个稳定可写的 HOME / cache
        Environment = [
          "HOME=/var/lib/xbot"
          "XDG_CACHE_HOME=/var/lib/xbot/cache"
        ];
        WorkingDirectory = "/var/lib/xbot";

        UMask = "0077";

        NoNewPrivileges = true;
        PrivateTmp = true;
        ProtectSystem = "strict";
        ProtectHome = true;

        ReadWritePaths = [ "/var/lib/xbot" videoDir ];
      };
    };
  };
}
