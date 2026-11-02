//! Mini GUI (kdialog): copies an installed Proton build into a separate compat tool with
//! dlssg-open (DLSS frame generation on RTX 30) installed, using the newest release.
//! Replaces the old bash script + the release's install.py, so no Python is needed.

use std::env;
use std::fs;
use std::io;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{exit, Command, Stdio};

// Fork of dniminenn/dlssg-open with the CUDA module API needed by dlssg_for_sm86 0.4.x
// (upstream PR: https://github.com/dniminenn/dlssg-open/pull/1). Back to upstream once that is released.
const REPO: &str = "ProfessorTee/dlssg-open";
const SM86_REPO: &str = "sdli1995/dlssg_for_sm86";
/// Proxy names dlssg_for_sm86 ships, in the order its alternatives/README says to try them.
/// dxgi.dll / d3d12.dll are left out: render-path proxies, often taken by ReShade & co.
const PROXIES: [&str; 4] = ["version", "winmm", "dbghelp", "dinput8"];
const TITLE: &str = "dlssg-open Proton";
const LAUNCH: &str = "DXVK_NVAPI_GPU_ARCH=AD100 NVCUDA_LITE_CC=8.9 DXVK_NVAPI_CUBIN_RETARGET_SM=86";
// With dlssg_for_sm86 (sdli1995) in the game folder: no arch spoof, the mod handles that
const LAUNCH_SM86: &str = "NVCUDA_LITE_CC=8.6 DXVK_NVAPI_CUBIN_RETARGET_SM=86 PROTON_ENABLE_NVAPI=1";

fn german() -> bool {
    ["LC_ALL", "LC_MESSAGES", "LANG"]
        .iter()
        .find_map(|k| env::var(k).ok().filter(|v| !v.is_empty()))
        .is_some_and(|v| v.starts_with("de"))
}

/// German UI on a German system, English otherwise.
fn t(de: &str, en: &str) -> String {
    if german() { de.into() } else { en.into() }
}

fn kdialog(args: &[&str]) -> Option<String> {
    let out = Command::new("kdialog").arg("--title").arg(TITLE).args(args).output().ok()?;
    out.status.success().then(|| String::from_utf8_lossy(&out.stdout).trim().to_string())
}

fn fail(msg: &str) -> ! {
    kdialog(&["--error", msg]);
    eprintln!("{msg}");
    exit(1)
}

fn run(cmd: &mut Command) -> bool {
    cmd.stdout(Stdio::null()).status().is_ok_and(|s| s.success())
}

fn home() -> PathBuf {
    PathBuf::from(env::var_os("HOME").unwrap_or_else(|| fail("HOME not set")))
}

/// Installed Proton builds that dlssg-open can patch (nvapi + nvcuda present), minus our own copies.
fn proton_builds(steam: &Path, dest: &Path) -> Vec<PathBuf> {
    let roots = [steam.join("steamapps/common"), PathBuf::from("/usr/share/steam/compatibilitytools.d"), dest.to_path_buf()];
    let mut found = Vec::new();
    for root in roots {
        let Ok(rd) = fs::read_dir(&root) else { continue };
        let mut dirs: Vec<PathBuf> = rd.flatten().map(|e| e.path()).collect();
        dirs.sort();
        for d in dirs {
            let name = d.file_name().unwrap_or_default().to_string_lossy().to_string();
            if root.ends_with("common") && !name.starts_with("Proton") {
                continue;
            }
            if name.contains("-dlssg-open") {
                continue;
            }
            if d.join("proton").is_file() && d.join("files/lib/wine/nvapi/x86_64-windows/nvapi64.dll").is_file() {
                found.push(d);
            }
        }
    }
    found
}

/// Newest release of `repo`: (tag, URL and sha256 digest of the first asset whose name ends with `suffix`).
fn latest_release(repo: &str, suffix: &str) -> (String, String, Option<String>) {
    let out = Command::new("curl")
        .args(["-fsSL", &format!("https://api.github.com/repos/{repo}/releases/latest")])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .unwrap_or_else(|| fail(&t("GitHub nicht erreichbar.", "GitHub not reachable.")));
    let json: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap_or_else(|_| fail("GitHub: invalid JSON"));
    let tag = json["tag_name"].as_str().unwrap_or_else(|| fail("GitHub: no tag_name")).to_string();
    let asset = json["assets"]
        .as_array()
        .and_then(|a| a.iter().find(|a| a["name"].as_str().is_some_and(|n| n.ends_with(suffix))))
        .unwrap_or_else(|| fail(&format!("{repo} {tag}: no {suffix} asset")));
    let url = asset["browser_download_url"].as_str().unwrap_or_else(|| fail("GitHub: no download URL")).to_string();
    let digest = asset["digest"].as_str().and_then(|d| d.strip_prefix("sha256:")).map(str::to_string);
    (tag, url, digest)
}

fn sha256(file: &Path) -> Option<String> {
    let out = Command::new("sha256sum").arg(file).output().ok()?;
    String::from_utf8_lossy(&out.stdout).split_whitespace().next().map(str::to_string)
}

/// Downloads + verifies the release into the cache once; returns the directory holding the DLLs.
fn fetch(cache: &Path, tag: &str, url: &str) -> PathBuf {
    let pkg = cache.join(tag);
    if !pkg.join(".ok").exists() {
        let _ = fs::remove_dir_all(&pkg);
        fs::create_dir_all(&pkg).unwrap_or_else(|e| fail(&e.to_string()));
        let tarball = pkg.join("release.tar.gz");
        if !run(Command::new("curl").args(["-fsSL", "-o"]).arg(&tarball).arg(url)) {
            fail(&t(&format!("Download von {tag} fehlgeschlagen."), &format!("Download of {tag} failed.")));
        }
        if !run(Command::new("tar").arg("-xzf").arg(&tarball).arg("-C").arg(&pkg)) {
            fail(&t(&format!("{tag} ließ sich nicht entpacken."), &format!("Could not extract {tag}.")));
        }
        let sums = find_file(&pkg, "SHA256SUMS").unwrap_or_else(|| fail(&format!("{tag}: no SHA256SUMS")));
        // SHA256SUMS lists paths relative to the directory above it (dlssg-open-x.y.z/...)
        let base = sums.parent().and_then(Path::parent).unwrap_or(&pkg);
        if !run(Command::new("sha256sum").args(["-c", "--quiet"]).arg(&sums).current_dir(base)) {
            fail(&t(&format!("Prüfsummen von {tag} stimmen nicht."), &format!("Checksums of {tag} do not match.")));
        }
        fs::write(pkg.join(".ok"), "").unwrap_or_else(|e| fail(&e.to_string()));
    }
    find_file(&pkg, "nvapi64.dll").and_then(|p| p.parent().map(Path::to_path_buf)).unwrap_or_else(|| fail(&format!("{tag}: no nvapi64.dll")))
}

fn find_file(dir: &Path, name: &str) -> Option<PathBuf> {
    for e in fs::read_dir(dir).ok()?.flatten() {
        let p = e.path();
        if p.file_name().is_some_and(|n| n == name) {
            return Some(p);
        }
        if p.is_dir() {
            if let Some(f) = find_file(&p, name) {
                return Some(f);
            }
        }
    }
    None
}

/// Same as dlssg-open's install.py --proton: keep Proton's file once as backup, then copy ours over it.
fn replace(dst: &Path, src: &Path, backup: &Path) -> io::Result<()> {
    if dst.symlink_metadata().is_ok() {
        if backup.exists() { fs::remove_file(dst)? } else { fs::rename(dst, backup)? }
    }
    fs::copy(src, dst)?;
    fs::set_permissions(dst, fs::Permissions::from_mode(0o755))
}

fn install(proton: &Path, payload: &Path) -> io::Result<()> {
    let w = proton.join("files/lib/wine");
    replace(&w.join("x86_64-windows/nvcuda.dll"), &payload.join("nvcuda.dll"), &w.join("x86_64-windows/nvcuda.dll.proton-stub"))?;
    replace(&w.join("nvapi/x86_64-windows/nvapi64.dll"), &payload.join("nvapi64.dll"), &w.join("nvapi/x86_64-windows/nvapi64.dll.proton-orig"))
}

fn main() {
    let steam = home().join(".local/share/Steam");
    let dest = steam.join("compatibilitytools.d");
    let cache = home().join(".cache/dlssg-open");

    match env::args().nth(1).as_deref() {
        Some("--folder") => {
            let _ = Command::new("xdg-open").arg(&dest).status();
            return;
        }
        Some("--game") => return install_sm86(&steam, &cache),
        Some("--proton") => {}
        _ => {
            let (a, b) = (t("Proton-Kopie mit dlssg-open erstellen", "Create a Proton copy with dlssg-open"), t("dlssg_for_sm86 in ein Spiel installieren / aktualisieren", "Install / update dlssg_for_sm86 in a game"));
            match kdialog(&["--menu", &t("Was soll gemacht werden?", "What do you want to do?"), "proton", &a, "game", &b]).as_deref() {
                Some("game") => return install_sm86(&steam, &cache),
                Some("proton") => {}
                _ => return,
            }
        }
    }

    let builds = proton_builds(&steam, &dest);
    if builds.is_empty() {
        fail(&t("Keine passende Proton-Version gefunden.", "No suitable Proton build found."));
    }
    let mut items: Vec<String> = Vec::new();
    for b in &builds {
        items.push(b.to_string_lossy().into());
        items.push(b.file_name().unwrap_or_default().to_string_lossy().into());
        items.push("off".into());
    }
    let question = t("Welche Proton-Version soll den Mod bekommen?", "Which Proton build should get the mod?");
    let mut args = vec!["--radiolist", question.as_str()];
    args.extend(items.iter().map(String::as_str));
    let Some(src) = kdialog(&args).filter(|s| !s.is_empty()) else { return };
    let src = PathBuf::from(src);
    let src_name = src.file_name().unwrap_or_default().to_string_lossy().to_string();

    let (tag, url, _) = latest_release(REPO, ".tar.gz");
    let payload = fetch(&cache, &tag, &url);

    let name = format!("{src_name}-dlssg-open-{tag}");
    let target = dest.join(&name);
    if target.exists() {
        let q = t(&format!("{name} existiert schon. Neu erstellen?"), &format!("{name} already exists. Recreate it?"));
        if kdialog(&["--warningyesno", &q]).is_none() {
            return;
        }
        fs::remove_dir_all(&target).unwrap_or_else(|e| fail(&e.to_string()));
    }

    let msg = t(&format!("Kopiere {src_name} … (kann etwas dauern)"), &format!("Copying {src_name} … (may take a while)"));
    kdialog(&["--passivepopup", &msg, "5"]);
    let _ = fs::create_dir_all(&dest);
    if !run(Command::new("nice").args(["-n", "19", "cp", "-a", "--reflink=auto"]).arg(&src).arg(&target)) {
        let _ = fs::remove_dir_all(&target);
        fail(&t("Kopieren fehlgeschlagen.", "Copy failed."));
    }

    // Own manifest so Steam lists the copy as a separate tool (Valve's toolmanifest.vdf stays for the runtime)
    let vdf = format!(
        "\"compatibilitytools\"\n{{\n  \"compat_tools\"\n  {{\n    \"{name}\"\n    {{\n      \"install_path\" \".\"\n      \"display_name\" \"{src_name} (dlssg-open {tag})\"\n      \"from_oslist\"  \"windows\"\n      \"to_oslist\"    \"linux\"\n    }}\n  }}\n}}\n"
    );
    if let Err(e) = fs::write(target.join("compatibilitytool.vdf"), vdf).and_then(|_| install(&target, &payload)) {
        let _ = fs::remove_dir_all(&target);
        fail(&format!("{}\n{e}", t("Installation fehlgeschlagen:", "Installation failed:")));
    }

    let done = t(
        &format!("Fertig: {src_name} (dlssg-open {tag})\n\n1. Steam neu starten\n2. Im Spiel unter Kompatibilität die neue Version wählen\n3. In die Launch-Optionen vor %command%:\n{LAUNCH}\n\nMit dlssg_for_sm86 (version.dll im Spielordner) stattdessen:\n{LAUNCH_SM86}\n\nNicht dazu: PROTON_NVIDIA_LIBS / PROTON_NVIDIA_NVCUDA"),
        &format!("Done: {src_name} (dlssg-open {tag})\n\n1. Restart Steam\n2. Select the new build under Compatibility in the game's properties\n3. Put this in the launch options before %command%:\n{LAUNCH}\n\nWith dlssg_for_sm86 (version.dll in the game folder) instead:\n{LAUNCH_SM86}\n\nDo not add: PROTON_NVIDIA_LIBS / PROTON_NVIDIA_NVCUDA"),
    );
    let (yes, no) = (t("Proton-Ordner öffnen", "Open Proton folder"), t("Schließen", "Close"));
    if kdialog(&["--yes-label", &yes, "--no-label", &no, "--yesno", &done]).is_some() {
        let _ = Command::new("xdg-open").arg(&dest).status();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn replace_keeps_first_backup() {
        let d = env::temp_dir().join(format!("dlssg-open-proton-test-{}", std::process::id()));
        let _ = fs::remove_dir_all(&d);
        fs::create_dir_all(&d).unwrap();
        let (dst, bak, a, b) = (d.join("x.dll"), d.join("x.dll.orig"), d.join("a"), d.join("b"));
        fs::write(&dst, "proton").unwrap();
        fs::write(&a, "ours1").unwrap();
        fs::write(&b, "ours2").unwrap();
        replace(&dst, &a, &bak).unwrap();
        replace(&dst, &b, &bak).unwrap();
        assert_eq!(fs::read_to_string(&bak).unwrap(), "proton");
        assert_eq!(fs::read_to_string(&dst).unwrap(), "ours2");
        fs::remove_dir_all(&d).unwrap();
    }
}

// ---------------------------------------------------------------------------------------------
// dlssg_for_sm86 (sdli1995) into a game folder

/// Steam library roots from libraryfolders.vdf (plus the default one).
fn steam_libraries(steam: &Path) -> Vec<PathBuf> {
    let mut libs = vec![steam.to_path_buf()];
    if let Ok(vdf) = fs::read_to_string(steam.join("steamapps/libraryfolders.vdf")) {
        for line in vdf.lines() {
            let f: Vec<&str> = line.split('"').collect();
            if f.len() >= 4 && f[1] == "path" {
                let p = PathBuf::from(f[3].replace("\\\\", "\\"));
                if !libs.iter().any(|l| fs::canonicalize(l).ok() == fs::canonicalize(&p).ok()) {
                    libs.push(p);
                }
            }
        }
    }
    libs
}

/// Value of `"key"  "value"` in a Steam .acf / .vdf text.
fn vdf_value(text: &str, key: &str) -> Option<String> {
    text.lines().find_map(|l| {
        let f: Vec<&str> = l.split('"').collect();
        (f.len() >= 4 && f[1] == key).then(|| f[3].to_string())
    })
}

/// Files below `dir` (at most `depth` levels) for which `keep` is true.
fn walk(dir: &Path, depth: usize, keep: &dyn Fn(&Path) -> bool, out: &mut Vec<PathBuf>) {
    let Ok(rd) = fs::read_dir(dir) else { return };
    for e in rd.flatten() {
        let p = e.path();
        let Ok(ft) = e.file_type() else { continue };
        if ft.is_dir() {
            // our own payload carries an nvngx_dlssg.dll too
            if depth > 0 && p.file_name().is_none_or(|n| n != "dlssg_sm86") {
                walk(&p, depth - 1, keep, out);
            }
        } else if keep(&p) {
            out.push(p);
        }
    }
}

fn name_is(p: &Path, name: &str) -> bool {
    p.file_name().is_some_and(|n| n.to_string_lossy().eq_ignore_ascii_case(name))
}

/// Installed Steam games that ship DLSS frame generation: (app id, name, install dir).
fn fg_games(steam: &Path) -> Vec<(String, String, PathBuf)> {
    let mut games = Vec::new();
    for lib in steam_libraries(steam) {
        let Ok(rd) = fs::read_dir(lib.join("steamapps")) else { continue };
        for e in rd.flatten() {
            let file = e.file_name().to_string_lossy().to_string();
            let Some(id) = file.strip_prefix("appmanifest_").and_then(|f| f.strip_suffix(".acf")) else { continue };
            let Ok(acf) = fs::read_to_string(e.path()) else { continue };
            let (Some(name), Some(dir)) = (vdf_value(&acf, "name"), vdf_value(&acf, "installdir")) else { continue };
            let dir = lib.join("steamapps/common").join(dir);
            let mut hits = Vec::new();
            walk(&dir, 9, &|p| name_is(p, "nvngx_dlssg.dll") || name_is(p, "sl.dlss_g.dll"), &mut hits);
            if !hits.is_empty() {
                games.push((id.to_string(), name, dir));
            }
        }
    }
    games.sort_by_key(|g| g.1.to_lowercase());
    games
}

/// The game's main EXE: the largest .exe that is not a crash handler, launcher stub, installer etc.
fn main_exe(game: &Path) -> Option<PathBuf> {
    const SKIP: [&str; 9] = ["crash", "unins", "redist", "setup", "eac", "anticheat", "report", "helper", "launcher"];
    // folders that hold side programs or the user's own copies, never the game EXE
    const SKIP_DIRS: [&str; 7] = ["epiconlineservices", "easyanticheat", "battleye", "redist", "/engine/", "_bak", "backup"];
    let mut exes = Vec::new();
    walk(game, 6, &|p| {
        let n = p.file_name().unwrap_or_default().to_string_lossy().to_lowercase();
        let rel = p.strip_prefix(game).unwrap_or(p).parent().map(|d| format!("/{}/", d.to_string_lossy().to_lowercase())).unwrap_or_default();
        n.ends_with(".exe") && !SKIP.iter().any(|s| n.contains(s)) && !SKIP_DIRS.iter().any(|s| rel.contains(s))
    }, &mut exes);
    exes.into_iter().max_by_key(|p| p.metadata().map(|m| m.len()).unwrap_or(0))
}

fn pick_exe(start: &Path) -> Option<PathBuf> {
    let filter = t("Spiel-EXE (*.exe *.EXE)", "Game EXE (*.exe *.EXE)");
    kdialog(&["--getopenfilename", &start.to_string_lossy(), &filter]).filter(|f| !f.is_empty()).map(PathBuf::from)
}

/// A proxy DLL of this mod (any name, 0.3.x and 0.4.x): they all carry "dlssg_sm86" as plain ASCII.
fn is_sm86_proxy(p: &Path) -> bool {
    fs::read(p).is_ok_and(|b| b.windows(10).any(|w| w == b"dlssg_sm86"))
}

/// Downloads + verifies + unpacks the newest dlssg_for_sm86 once; returns the folder holding dlssg_sm86.ini.
fn fetch_sm86(cache: &Path) -> (String, PathBuf) {
    let (tag, url, digest) = latest_release(SM86_REPO, "-x64.zip");
    let pkg = cache.join(format!("dlssg_for_sm86-{tag}"));
    if !pkg.join(".ok").exists() {
        let _ = fs::remove_dir_all(&pkg);
        fs::create_dir_all(&pkg).unwrap_or_else(|e| fail(&e.to_string()));
        let zip = pkg.join("release.zip");
        kdialog(&["--passivepopup", &t(&format!("Lade dlssg_for_sm86 {tag} …"), &format!("Downloading dlssg_for_sm86 {tag} …")), "5"]);
        if !run(Command::new("curl").args(["-fsSL", "-o"]).arg(&zip).arg(&url)) {
            fail(&t(&format!("Download von dlssg_for_sm86 {tag} fehlgeschlagen."), &format!("Download of dlssg_for_sm86 {tag} failed.")));
        }
        if let Some(d) = digest {
            if sha256(&zip).as_deref() != Some(d.as_str()) {
                let _ = fs::remove_dir_all(&pkg);
                fail(&t(&format!("Prüfsumme von dlssg_for_sm86 {tag} stimmt nicht."), &format!("Checksum of dlssg_for_sm86 {tag} does not match.")));
            }
        }
        let unpacked = run(Command::new("unzip").args(["-q", "-o"]).arg(&zip).arg("-d").arg(&pkg))
            || run(Command::new("bsdtar").arg("-xf").arg(&zip).arg("-C").arg(&pkg));
        if !unpacked {
            fail(&t("Entpacken fehlgeschlagen (unzip oder bsdtar nötig).", "Extraction failed (needs unzip or bsdtar)."));
        }
        let _ = fs::remove_file(&zip);
        fs::write(pkg.join(".ok"), "").unwrap_or_else(|e| fail(&e.to_string()));
    }
    let root = find_file(&pkg, "dlssg_sm86.ini").and_then(|p| p.parent().map(Path::to_path_buf));
    (tag.clone(), root.unwrap_or_else(|| fail(&format!("dlssg_for_sm86 {tag}: no dlssg_sm86.ini"))))
}

/// The mod loaded as ASI plugin (e.g. 00_dlssg_sm86.asi next to an ASI loader), if present: updated in place.
fn sm86_asi(dir: &Path) -> Option<PathBuf> {
    fs::read_dir(dir).ok()?.flatten().map(|e| e.path()).find(|p| p.extension().is_some_and(|x| x.eq_ignore_ascii_case("asi")) && is_sm86_proxy(p))
}

/// Which proxy name to use in `dir`: the first name that is free or already ours; None if all are taken.
fn pick_proxy(dir: &Path) -> Option<&'static str> {
    PROXIES.into_iter().find(|n| {
        let p = dir.join(format!("{n}.dll"));
        !p.exists() || is_sm86_proxy(&p)
    })
}

/// Copies the mod into `dir` (upgrade-safe: removes old proxies of the mod, keeps the old INI as .bak).
/// `proxy` is a DLL name from PROXIES, or the file name of an existing ASI of the mod.
fn place_sm86(src: &Path, dir: &Path, proxy: &str) -> io::Result<()> {
    for n in PROXIES {
        let p = dir.join(format!("{n}.dll"));
        if p.exists() && is_sm86_proxy(&p) {
            fs::remove_file(&p)?;
        }
    }
    let ini = dir.join("dlssg_sm86.ini");
    if ini.exists() {
        fs::rename(&ini, dir.join("dlssg_sm86.ini.bak"))?;
    }
    let payload = dir.join("dlssg_sm86");
    if payload.exists() {
        fs::remove_dir_all(&payload)?;
    }
    let (proxy_src, proxy_dst) = match proxy {
        p if p.ends_with(".asi") => (src.join("version.dll"), dir.join(p)),
        "version" => (src.join("version.dll"), dir.join("version.dll")),
        p => (src.join("alternatives").join(format!("{p}.dll")), dir.join(format!("{p}.dll"))),
    };
    fs::copy(&proxy_src, &proxy_dst)?;
    fs::copy(src.join("dlssg_sm86.ini"), &ini)?;
    if !run(Command::new("cp").args(["-a", "--reflink=auto"]).arg(src.join("dlssg_sm86")).arg(dir)) {
        return Err(io::Error::other("cp dlssg_sm86 failed"));
    }
    Ok(())
}

fn install_sm86(steam: &Path, cache: &Path) {
    let games = fg_games(steam);
    if games.is_empty() {
        fail(&t("Kein installiertes Steam-Spiel mit DLSS Frame Generation gefunden.", "No installed Steam game with DLSS Frame Generation found."));
    }
    let mut items: Vec<String> = Vec::new();
    for (i, (id, name, _)) in games.iter().enumerate() {
        items.extend([i.to_string(), format!("{name} ({id})"), "off".into()]);
    }
    items.extend(["exe".into(), t("EXE selbst auswählen …", "Choose an EXE myself …"), "off".into()]);
    let q = t("In welches Spiel soll dlssg_for_sm86 (neu oder Update)?", "Which game should get dlssg_for_sm86 (new or update)?");
    let mut args = vec!["--radiolist", q.as_str()];
    args.extend(items.iter().map(String::as_str));
    let Some(choice) = kdialog(&args) else { return };

    let exe = match choice.parse::<usize>().ok().and_then(|i| games.get(i)) {
        Some((_, name, gdir)) => {
            let Some(found) = main_exe(gdir) else { return install_next_to(pick_exe(gdir), cache) };
            let q = t(&format!("{name}\n\nGefundene Spiel-EXE:\n{}\n\nDaneben installieren?", found.display()), &format!("{name}\n\nGame EXE found:\n{}\n\nInstall next to it?", found.display()));
            let (yes, no) = (t("Installieren", "Install"), t("Andere EXE …", "Other EXE …"));
            match Command::new("kdialog").args(["--title", TITLE, "--yes-label", &yes, "--no-label", &no, "--yesnocancel", &q]).status().ok().and_then(|s| s.code()) {
                Some(0) => Some(found),
                Some(1) => pick_exe(found.parent().unwrap_or(gdir)),
                _ => return,
            }
        }
        None if choice == "exe" => pick_exe(&steam.join("steamapps/common")),
        None => return,
    };
    install_next_to(exe, cache);
}

/// Installs or updates dlssg_for_sm86 next to `exe`.
fn install_next_to(exe: Option<PathBuf>, cache: &Path) {
    let Some(exe) = exe else { return };
    let Some(dir) = exe.parent().map(Path::to_path_buf) else { return };
    let name = exe.file_name().unwrap_or_default().to_string_lossy().to_string();

    let asi = sm86_asi(&dir).map(|p| p.file_name().unwrap_or_default().to_string_lossy().to_string());
    let proxy: String = match asi {
        Some(a) => a,
        None => pick_proxy(&dir).unwrap_or_else(|| fail(&t("version.dll, winmm.dll, dbghelp.dll und dinput8.dll sind dort schon von anderen Mods belegt.", "version.dll, winmm.dll, dbghelp.dll and dinput8.dll are already taken by other mods there."))).to_string(),
    };
    let (tag, src) = fetch_sm86(cache);
    if let Err(e) = place_sm86(&src, &dir, &proxy) {
        fail(&format!("{}\n{e}", t("Installation fehlgeschlagen:", "Installation failed:")));
    }

    // an ASI is loaded by the game's ASI loader, which needs its own override already
    let opts = if proxy.ends_with(".asi") { format!("{LAUNCH_SM86} %command%") } else { format!("{LAUNCH_SM86} WINEDLLOVERRIDES=\"{proxy}=n,b\" %command%") };
    let shown_proxy = if proxy.ends_with(".asi") { proxy.clone() } else { format!("{proxy}.dll") };
    let bak = if dir.join("dlssg_sm86.ini.bak").exists() { t("\n\nDie alte dlssg_sm86.ini liegt als dlssg_sm86.ini.bak daneben.", "\n\nThe old dlssg_sm86.ini was kept as dlssg_sm86.ini.bak.") } else { String::new() };
    let done = t(
        &format!("Fertig: dlssg_for_sm86 {tag} ({shown_proxy}) in {name}\n\n1. Im Spiel unter Kompatibilität eine dlssg-open-Proton-Kopie wählen\n2. Launch-Optionen (vorhandene wie mangohud davor lassen):\n{opts}\n\nKein DXVK_NVAPI_GPU_ARCH=AD100 dazu.{bak}"),
        &format!("Done: dlssg_for_sm86 {tag} ({shown_proxy}) in {name}\n\n1. Select a dlssg-open Proton copy under Compatibility in the game's properties\n2. Launch options (keep existing ones like mangohud in front):\n{opts}\n\nDo not add DXVK_NVAPI_GPU_ARCH=AD100.{bak}"),
    );
    kdialog(&["--msgbox", &done]);
}

#[cfg(test)]
mod sm86_tests {
    use super::*;

    #[test]
    fn proxy_choice_skips_foreign_dlls_and_reuses_ours() {
        let d = env::temp_dir().join(format!("dlssg-open-proton-proxy-{}", std::process::id()));
        let _ = fs::remove_dir_all(&d);
        fs::create_dir_all(&d).unwrap();
        assert_eq!(pick_proxy(&d), Some("version"));
        fs::write(d.join("version.dll"), "some other mod").unwrap();
        assert_eq!(pick_proxy(&d), Some("winmm"));
        fs::write(d.join("winmm.dll"), "xx dlssg_sm86\\backend xx").unwrap();
        assert_eq!(pick_proxy(&d), Some("winmm"));
        for n in ["winmm", "dbghelp", "dinput8"] {
            fs::write(d.join(format!("{n}.dll")), "foreign").unwrap();
        }
        assert_eq!(pick_proxy(&d), None);
        assert_eq!(sm86_asi(&d), None);
        fs::write(d.join("CrimsonRoute.asi"), "foreign").unwrap();
        fs::write(d.join("00_dlssg_sm86.asi"), "xx dlssg_sm86\\backend xx").unwrap();
        assert_eq!(sm86_asi(&d), Some(d.join("00_dlssg_sm86.asi")));
        fs::remove_dir_all(&d).unwrap();
    }

    #[test]
    fn vdf_value_reads_quoted_pairs() {
        let acf = "\"AppState\"\n{\n\t\"appid\"\t\t\"2183900\"\n\t\"name\"\t\t\"Space Marine 2\"\n\t\"installdir\"\t\t\"Space Marine 2\"\n}";
        assert_eq!(vdf_value(acf, "name").as_deref(), Some("Space Marine 2"));
        assert_eq!(vdf_value(acf, "installdir").as_deref(), Some("Space Marine 2"));
        assert_eq!(vdf_value(acf, "missing"), None);
    }
}
