# Source Runtime Profiles

Purpose: source-controlled, typed, secret-free runtime and deployment profile inputs.

Owner: SDKWork Sandbox runtime configuration maintainers.

Allowed: safe defaults, schemas, profile templates, and secret references. Forbidden: concrete production endpoints, passwords, access tokens, private keys, local overrides, logs, caches, and runtime databases.

Related specs: `../../sdkwork-specs/SOURCE_CONFIG_SPEC.md`, `../../sdkwork-specs/CONFIG_SPEC.md`, `../../sdkwork-specs/RUNTIME_DIRECTORY_SPEC.md`.

Verification: `node ../sdkwork-specs/tools/check-source-config-standard.mjs --root .`. The deployment profile index and its four `topology/*.env` profiles are in place, and the gate passes. It is part of `pnpm run check`. A green gate proves source-config consistency only, not a running Sandbox or a deployable host.

<!-- SDKWORK-DEPLOY-LAYOUT: v1 -->
## Installed Runtime Paths

Authority: `APPLICATION_DEPLOY_LAYOUT_SPEC.md` (`../sdkwork-specs/`).

| Item | Value |
| --- | --- |
| `appId` | `sdkwork-sandbox` |
| `runtimeCode` | `sandbox` |
| Config root | `/etc/sdkwork/sandbox/` |
| Runtime TOML | `/etc/sdkwork/sandbox/config.toml` |
| Secrets | `/etc/sdkwork/sandbox/secrets/` |
| Override | `SDKWORK_SANDBOX_CONFIG_FILE` |

Source profiles live under `etc/` (`sdkwork.deployment.config.json` index). Deploy manifest: `deployments/deploy.yaml`. Web data-plane source: `deployments/webserver/` (`SDKWORK_WEBSERVER_SPEC.md` layout v3).

```bash
node ../sdkwork-specs/tools/check-source-config-standard.mjs --root .
node ../sdkwork-specs/tools/check-application-deploy-layout.mjs --root .
node ../sdkwork-specs/tools/check-webserver-toml-standard.mjs --root .
```
<!-- /SDKWORK-DEPLOY-LAYOUT -->

