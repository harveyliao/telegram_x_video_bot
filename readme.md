#

## encrypt cookie

`nix-shell -p sops ssh-to-age`
`cat /etc/ssh/ssh_host_ed25519_key.pub | ssh-to-age`
