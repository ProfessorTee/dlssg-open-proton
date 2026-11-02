# dlssg-open Proton

**English** · [Deutsch](README.de.md)

**Small KDE tool that copies an installed Proton build and installs
[dlssg-open](https://github.com/dniminenn/dlssg-open) into the copy.** This gets DLSS Frame Generation
running on RTX 30 cards (Ampere) on Linux. The original Proton build stays untouched; Steam lists the copy
as a separate compatibility tool, e.g. "GE-Proton11-11 (dlssg-open v0.1.1)". So only the games you switch
to the copy get the mod.

> **Currently uses the fork [ProfessorTee/dlssg-open](https://github.com/ProfessorTee/dlssg-open)**, release
> 0.1.1: upstream dlssg-open 0.1.0 plus the CUDA module API (`CreateCuModule` / `CreateCuFunction` / `LaunchCuKernelChain`) that
> [dlssg_for_sm86](https://github.com/sdli1995/dlssg_for_sm86) 0.4.x needs. Upstream PR:
> [dniminenn/dlssg-open#1](https://github.com/dniminenn/dlssg-open/pull/1). The tool switches back to upstream once
> that is released.

## Usage

1. Open **Create dlssg-open Proton** from the start menu (or run `dlssg-open-proton` in a terminal) and choose
   *Create a Proton copy with dlssg-open*.
2. Pick the Proton build that should get the mod.
3. Restart Steam, then select the new build in the game under *Properties → Compatibility*.
4. Add to the game's launch options:

   ```
   DXVK_NVAPI_GPU_ARCH=AD100 NVCUDA_LITE_CC=8.9 DXVK_NVAPI_CUBIN_RETARGET_SM=86 %command%
   ```

   With **dlssg_for_sm86** (the sdli1995 mod: `version.dll`, `dlssg_sm86.ini` and `dlssg_sm86\` in the game
   folder) use this instead, without the architecture spoof (the mod handles that, and gets 3X / 4X):

   ```
   NVCUDA_LITE_CC=8.6 DXVK_NVAPI_CUBIN_RETARGET_SM=86 PROTON_ENABLE_NVAPI=1 %command%
   ```

   If the game already has its own `version.dll`, add `WINEDLLOVERRIDES="version=n,b"` or use one of the
   mod's `alternatives\` proxies.

   Do **not** set `PROTON_NVIDIA_LIBS` or `PROTON_NVIDIA_NVCUDA`, otherwise the real nvcuda is loaded
   instead of nvcuda-lite.

Right-click the start menu entry → *Open Proton folder* opens `compatibilitytools.d` so you can delete old
copies. The interface follows the system language: German on German systems, English otherwise.

## dlssg_for_sm86 into a game

The second menu entry (also *right-click the start menu entry → Install / update dlssg_for_sm86 in a game*, or
`dlssg-open-proton --game`) puts the newest [dlssg_for_sm86](https://github.com/sdli1995/dlssg_for_sm86) release
next to a game's EXE, or updates it there:

- Lists the installed Steam games that ship DLSS Frame Generation and finds their main EXE; *Other EXE …* or
  *Choose an EXE myself …* opens a file dialog, so non-Steam games work too.
- Downloads the release once (checked against GitHub's SHA-256), keeps it in `~/.cache/dlssg-open/`.
- Copies the proxy, `dlssg_sm86.ini` and `dlssg_sm86\` next to the EXE. The proxy is `version.dll`; if another mod
  already uses that name, the next free one of `winmm.dll`, `dbghelp.dll`, `dinput8.dll` (the order the mod's
  `alternatives\README.md` gives). Only one proxy of the mod stays in the folder.
- Update: old proxies of the mod are removed, the old INI is kept as `dlssg_sm86.ini.bak` (redo your edits in the
  new one). If the mod is loaded as an ASI plugin (e.g. `00_dlssg_sm86.asi` next to an ASI loader), that file is
  updated in place.
- Shows the launch options to use, including the `WINEDLLOVERRIDES` for the chosen proxy.

## What the tool does

- Lists every installed Proton build with dxvk-nvapi (Steam's Proton, system-wide ones and your own in
  `compatibilitytools.d`), excluding its own dlssg-open copies.
- Downloads the **newest release** of dlssg-open via the GitHub API, checks its `SHA256SUMS` and keeps it in
  `~/.cache/dlssg-open/<tag>`.
- Copies the chosen Proton build to `compatibilitytools.d/<name>-dlssg-open-<tag>` (with `--reflink`, so on
  btrfs it takes almost no space), writes its own `compatibilitytool.vdf` and puts dlssg-open's `nvapi64.dll` and
  `nvcuda.dll` into the copy (Proton's originals stay next to them as `.proton-orig` / `.proton-stub`, the same
  as dlssg-open's `install.py --proton` does). If that fails, the copy is deleted again.

When a new dlssg-open or Proton release comes out, just run the tool again.

## Requirements

- Steam (native, `~/.local/share/Steam`), KDE (`kdialog`), `curl`, `tar`, `sha256sum`; no Python (since 2.0
  the tool is a single Rust binary)
- RTX 30 and a game with DLSS-G runtime 310.x – details and limits in the
  [dlssg-open README](https://github.com/ProfessorTee/dlssg-open/tree/cu-module-api#readme)
- dlssg_for_sm86 0.4.x needs dlssg-open 0.1.1 (the fork above); with 0.1.0 frame generation fails or
  runs NVIDIA's stock kernels instead of the mod's. Tested: 0.4.1 in Monster Hunter Wilds and Crimson
  Desert, RTX 3090, 4X.

## Installation

```
git clone https://github.com/ProfessorTee/dlssg-open-proton.git
cd dlssg-open-proton
./install.sh
```

`install.sh` builds the tool when Rust (`cargo`) is installed, otherwise it downloads the prebuilt binary of
the latest release (x86_64, glibc ≥ 2.35).

Uninstall: delete `~/.local/bin/dlssg-open-proton` and
`~/.local/share/applications/dlssg-open-proton.desktop`.

## License

MIT. dlssg-open itself is a separate project by dniminenn (also MIT); the tool downloads it from the
GitHub releases of the fork at runtime, it is not distributed with this repo.
