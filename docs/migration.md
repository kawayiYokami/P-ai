# Migration Policy

## Current Strategy

- Project is still in early stage.
- Old config compatibility is **not required**.
- On invalid or missing key fields, app normalizes config to a safe state.

## Normalization Rules

1. If `apiConfigs` is empty, use default config.
2. If `selectedApiConfigId` is invalid, fallback to first API config.
3. If `chatApiConfigId` is invalid or not text-capable, fallback to first text-capable API config.
4. If `sttApiConfigId` is invalid or not audio-capable, clear it.
5. If `visionApiConfigId` is invalid or not image-capable, clear it.

## Operational Impact

- `chatApiConfigId` is the only default source for chat runtime routing.
- Switching "currently editing API config" no longer changes chat runtime API.
