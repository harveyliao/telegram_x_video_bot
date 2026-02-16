#

## encrypt cookie

`nix-shell -p sops ssh-to-age`
`cat /etc/ssh/ssh_host_ed25519_key.pub | ssh-to-age`

## docs

- SOPS on NixOS troubleshooting: `docs/sops-nixos-troubleshooting.md`

## Video archive

Bot download output format:
- Task directory: `VIDEO_DIR/YYYY-MM-DD_HH-MM-SS_<chat_id>/`
- Video file: `video.mp4`

Archive script:
- Repo path: `scripts/xbot-video-archive.sh`
- Runtime path on host: `/var/lib/xbot/bin/xbot-video-archive.sh`
- Behavior: move `video.mp4` to `/data/xbot/videos/video_YYYY-MM-DD_HH-MM-SS.mp4`
- Collision handling: append `_1`, `_2`, ...
- Cleanup: remove stale task directories that only contain `cookie.txt`

### Permission setup

If the bot runs as user `xbot`:

```bash
sudo install -d -o xbot -g xbot -m 0750 /data/xbot/videos
```

If folder already exists and is root-owned:

```bash
sudo chown xbot:xbot /data/xbot/videos
sudo chmod 0750 /data/xbot/videos
```

### Install script to runtime path

```bash
sudo install -d -o xbot -g xbot -m 0750 /var/lib/xbot/bin
sudo install -o xbot -g xbot -m 0750 ./scripts/xbot-video-archive.sh /var/lib/xbot/bin/xbot-video-archive.sh
```

### NixOS systemd timer (recommended)

Put this in `/etc/nixos/xbot-archive.nix` and ensure it is imported by `/etc/nixos/configuration.nix`:

```nix
{ pkgs, ... }:

{
  systemd.services.xbot-video-archive = {
    description = "Archive downloaded xbot videos";

    path = [
      pkgs.bash
      pkgs.coreutils
      pkgs.findutils
    ];

    serviceConfig = {
      Type = "oneshot";
      User = "xbot";
      Group = "xbot";
      ExecStart = "/var/lib/xbot/bin/xbot-video-archive.sh";
      WorkingDirectory = "/";
    };

    environment = {
      SRC_DIR = "/var/lib/xbot/videos";
      DST_DIR = "/data/xbot/videos";
    };
  };

  systemd.timers.xbot-video-archive = {
    wantedBy = [ "timers.target" ];
    timerConfig = {
      OnCalendar = "*:0/5"; # every 5 minutes
      Persistent = true;
      Unit = "xbot-video-archive.service";
    };
  };
}
```

Apply and verify:

```bash
cd /etc/nixos
sudo nix flake lock --update-input xbot
sudo nixos-rebuild switch --flake .#hpworkstation
systemctl status xbot-video-archive.timer
systemctl list-timers | rg xbot-video-archive
```

### Manual test and logs

```bash
sudo systemctl start xbot-video-archive.service
journalctl -u xbot-video-archive.service -n 50 --no-pager
```
