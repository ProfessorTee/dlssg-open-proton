# dlssg-open Proton

**English** · [Deutsch](README.de.md)

**Small KDE tool that copies an installed Proton build and installs
[dlssg-open](https://github.com/dniminenn/dlssg-open) into the copy.** This gets DLSS Frame Generation
running on RTX 30 cards (Ampere) on Linux. The original Proton build stays untouched; Steam lists the copy
as a separate compatibility tool, e.g. "GE-Proton11-11 (dlssg-open v0.1.0)". So only the games you switch
to the copy get the mod.

## Usage

1. Open **Create dlssg-open Proton** from the start menu (or run `dlssg-open-proton` in a terminal).
2. Pick the Proton build that should get the mod.
3. Restart Steam, then select the new build in the game under *Properties → Compatibility*.
4. Add to the game's launch options:

   ```
   DXVK_NVAPI_GPU_ARCH=AD100 NVCUDA_LITE_CC=8.9 DXVK_NVAPI_CUBIN_RETARGET_SM=86 %command%
   ```

   Do **not** set `PROTON_NVIDIA_LIBS` or `PROTON_NVIDIA_NVCUDA`, otherwise the real nvcuda is loaded
   instead of nvcuda-lite.

Right-click the start menu entry → *Open Proton folder* opens `compatibilitytools.d` so you can delete old
copies. The interface follows the system language: German on German systems, English otherwise.

## What the script does

- Lists every installed Proton build with dxvk-nvapi (Steam's Proton, system-wide ones and your own in
  `compatibilitytools.d`), excluding its own dlssg-open copies.
- Downloads the **newest release** of dlssg-open via the GitHub API, checks its `SHA256SUMS` and keeps it in
  `~/.cache/dlssg-open/<tag>`.
- Copies the chosen Proton build to `compatibilitytools.d/<name>-dlssg-open-<tag>` (with `--reflink`, so on
  btrfs it takes almost no space), writes its own `compatibilitytool.vdf` and runs the dlssg-open installer on
  the copy. If that fails, the copy is deleted again.

When a new dlssg-open or Proton release comes out, just run the tool again.

## Requirements

- Steam (native, `~/.local/share/Steam`), KDE (`kdialog`), `curl`, `python3`
- RTX 30 and a game with DLSS-G runtime 310.x – details and limits in the
  [dlssg-open README](https://github.com/dniminenn/dlssg-open#readme)

## Installation

```
git clone https://github.com/ProfessorTee/dlssg-open-proton.git
cd dlssg-open-proton
./install.sh
```

Uninstall: delete `~/.local/bin/dlssg-open-proton` and
`~/.local/share/applications/dlssg-open-proton.desktop`.

## License

MIT. dlssg-open itself is a separate project by dniminenn (also MIT) and is downloaded from its GitHub
releases at runtime, not distributed with this repo.
