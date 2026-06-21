# Release signing setup

## Required GitHub Actions secrets

Set these in the repository **Settings → Secrets and variables → Actions**:

| Secret                               | Description                                                                                                                |
| ------------------------------------ | -------------------------------------------------------------------------------------------------------------------------- |
| `TAURI_SIGNING_PRIVATE_KEY`          | Tauri updater signing key (minisign private key). Generate with `pnpm tauri signer generate`. Copy the private key output. |
| `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` | Password chosen when running `tauri signer generate`.                                                                      |
| `APPLE_CERTIFICATE`                  | Base64-encoded `.p12` export of a Developer ID Application certificate from Keychain.                                      |
| `APPLE_CERTIFICATE_PASSWORD`         | Password set when exporting the `.p12`.                                                                                    |
| `APPLE_SIGNING_IDENTITY`             | E.g. `Developer ID Application: Your Name (TEAMID)`.                                                                       |
| `APPLE_ID`                           | Apple ID email used for notarization.                                                                                      |
| `APPLE_PASSWORD`                     | App-specific password for the Apple ID (generated at appleid.apple.com).                                                   |
| `APPLE_TEAM_ID`                      | 10-character team identifier from developer.apple.com.                                                                     |
| `WINDOWS_CERTIFICATE`                | Base64-encoded `.pfx` Authenticode certificate.                                                                            |
| `WINDOWS_CERTIFICATE_PASSWORD`       | Password for the `.pfx`.                                                                                                   |

## Generating the Tauri updater key pair

```sh
pnpm tauri signer generate
```

Output includes a **public key** and a **private key**. Copy the public key into
`tauri.conf.json` → `plugins.updater.pubkey`. Store the private key as
`TAURI_SIGNING_PRIVATE_KEY` in CI secrets.

## Update manifest

`tauri-apps/tauri-action` with `includeUpdaterJson: true` automatically generates
`latest.json` and uploads it alongside the release artifacts. The Synapse update
checker fetches:

```
https://github.com/synapse-srs/synapse/releases/latest/download/latest.json
```

## Local signing test

```sh
# Sign a single binary manually:
pnpm tauri signer sign -k <private-key-path> <path-to-binary>
```
