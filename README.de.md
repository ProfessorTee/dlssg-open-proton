# dlssg-open Proton

[English](README.md) · **Deutsch**

**Kleines KDE-Tool, das eine installierte Proton-Version kopiert und in die Kopie
[dlssg-open](https://github.com/dniminenn/dlssg-open) einbaut.** Damit läuft DLSS Frame Generation
auf RTX-30-Karten (Ampere) unter Linux. Die Original-Proton-Version bleibt unverändert, Steam zeigt die
Kopie als eigenes Kompatibilitätstool an, z. B. „GE-Proton11-11 (dlssg-open v0.1.0)“. So bekommen nur
die Spiele den Mod, bei denen du die Kopie auswählst.

## Bedienung

1. Im Startmenü **dlssg-open Proton erstellen** öffnen (oder `dlssg-open-proton` im Terminal).
2. Die Proton-Version auswählen, die den Mod bekommen soll.
3. Steam neu starten, im Spiel unter *Eigenschaften → Kompatibilität* die neue Version wählen.
4. In die Startoptionen des Spiels:

   ```
   DXVK_NVAPI_GPU_ARCH=AD100 NVCUDA_LITE_CC=8.9 DXVK_NVAPI_CUBIN_RETARGET_SM=86 %command%
   ```

   `PROTON_NVIDIA_LIBS` und `PROTON_NVIDIA_NVCUDA` **nicht** setzen, sonst wird das echte nvcuda
   statt nvcuda-lite geladen.

Rechtsklick auf den Startmenü-Eintrag → *Proton-Ordner öffnen* zeigt `compatibilitytools.d`, um alte
Kopien zu löschen. Die Oberfläche folgt der Systemsprache: Deutsch auf deutschen Systemen, sonst Englisch.

## Was das Skript macht

- Listet alle installierten Proton-Builds mit dxvk-nvapi (Steam-Proton, systemweite und eigene in
  `compatibilitytools.d`), eigene dlssg-open-Kopien ausgenommen.
- Lädt das **neueste Release** von dlssg-open über die GitHub-API, prüft die `SHA256SUMS` und
  speichert es in `~/.cache/dlssg-open/<tag>`.
- Kopiert die gewählte Proton-Version nach `compatibilitytools.d/<name>-dlssg-open-<tag>`
  (mit `--reflink`, auf btrfs also fast ohne Platz), legt eine eigene `compatibilitytool.vdf` an und
  führt den Installer von dlssg-open auf der Kopie aus. Schlägt das fehl, wird die Kopie wieder gelöscht.

Kommt ein neues dlssg-open- oder Proton-Release, das Tool einfach nochmal ausführen.

## Voraussetzungen

- Steam (nativ, `~/.local/share/Steam`), KDE (`kdialog`), `curl`, `python3`
- RTX 30 und ein Spiel mit DLSS-G-Runtime 310.x – Details und Grenzen im
  [dlssg-open-README](https://github.com/dniminenn/dlssg-open#readme)

## Installation

```
git clone https://github.com/ProfessorTee/dlssg-open-proton.git
cd dlssg-open-proton
./install.sh
```

Deinstallieren: `~/.local/bin/dlssg-open-proton` und
`~/.local/share/applications/dlssg-open-proton.desktop` löschen.

## Lizenz

MIT. dlssg-open selbst ist ein eigenes Projekt von dniminenn (ebenfalls MIT) und wird zur Laufzeit von
dessen GitHub-Releases geladen, nicht mit diesem Repo verteilt.
