# SOPS on NixOS: Troubleshooting and Safe Workflow

This project uses SOPS + age recipients for `secrets/xbot.yaml`.

## Common failure we hit

When running:

```bash
nix-shell -p sops --run 'sops secrets/xbot.yaml'
```

You may get:

```text
no identity matched any of the recipients
Did not find keys in locations 'SOPS_AGE_SSH_PRIVATE_KEY_FILE', '/home/<user>/.ssh/id_rsa', 'SOPS_AGE_KEY', 'SOPS_AGE_KEY_FILE', ...
```

## Why this happens

1. The file is encrypted to age recipients you do not have private keys for.
2. SOPS does not auto-detect `~/.ssh/id_ed25519` unless you set `SOPS_AGE_SSH_PRIVATE_KEY_FILE`.
3. On NixOS, if you use `nix-shell`, you still need env vars pointing to the right key material.

## Recommended setup (works on NixOS)

Use a persistent age key file:

```bash
export SOPS_AGE_KEY_FILE="$HOME/.config/sops/age/key.txt"
```

Then edit:

```bash
nix-shell -p sops --run 'sops secrets/xbot.yaml'
```

Alternative (SSH private key mode):

```bash
export SOPS_AGE_SSH_PRIVATE_KEY_FILE="$HOME/.ssh/id_ed25519"
nix-shell -p sops --run 'sops secrets/xbot.yaml'
```

## Verify your recipients

SSH pubkey -> age recipient:

```bash
nix-shell -p ssh-to-age --run 'cat ~/.ssh/id_ed25519.pub | ssh-to-age'
```

age key file -> age recipient:

```bash
nix-shell -p age --run 'age-keygen -y ~/.config/sops/age/key.txt'
```

Check recipients in encrypted file:

```bash
rg -n 'recipient:' secrets/xbot.yaml
```

## Team-safe recipient management

Keep recipients in `.sops.yaml` so future encryption is consistent:

```yaml
creation_rules:
  - path_regex: ^secrets/.*\.yaml$
    age: age1...,age1...,age1...
```

After changing `.sops.yaml`, if `sops updatekeys` fails due to decrypt issues, regenerate from plaintext source:

```bash
nix-shell -p sops --run \
  'sops encrypt --filename-override secrets/xbot.yaml secrets/xbot.yaml.raw > secrets/xbot.yaml'
```

If needed, pass recipients explicitly:

```bash
nix-shell -p sops --run \
  'sops encrypt --age "age1...,age1..." --filename-override secrets/xbot.yaml secrets/xbot.yaml.raw > secrets/xbot.yaml'
```

## For people cloning this repo (own deployment)

If you deploy your own bot instance, do not keep someone else's recipient unless you intentionally want them to decrypt your secrets.

Recommended steps:

1. Add your own recipient(s) to `.sops.yaml`.
2. Remove the original maintainer recipient(s) from `.sops.yaml`.
3. Re-encrypt `secrets/xbot.yaml` so only your private key(s) can decrypt.

Quick check:

```bash
rg -n 'recipient:' secrets/xbot.yaml
```

If an old recipient still appears there, that person can still decrypt the file.

## Key name gotcha in this repo

The token key should be:

```yaml
teloxide_token: "..."
```

and `nix/module.nix` must use:

```nix
key = "teloxide_token";
```

If one side says `teoloxide_token` (typo) and the other says `teloxide_token`, secret lookup breaks.

## Security notes

1. `secrets/xbot.yaml.raw` is plaintext; treat it as highly sensitive.
2. `secrets/` is currently gitignored in this repo, so encrypted secret updates are not committed by default.
3. If a token/cookie was exposed in plaintext or logs, rotate it immediately.
