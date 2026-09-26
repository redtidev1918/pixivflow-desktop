# assets

Source/branding assets live here. Build-critical platform icons are **generated**
into `src-tauri/icons/` by Tauri and are not edited by hand — regenerate them
from the master source:

```bash
npm run icon   # tauri icon assets/icon.png  -> writes src-tauri/icons/
```

| Path | Purpose |
|---|---|
| `icon.png` | Master app icon. `tauri icon` derives all platform formats (icns/ico/ico sets) from it. |
| *(future)* | dmg background, installer banners, README banner art. |