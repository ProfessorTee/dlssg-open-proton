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

/// Newest release: (tag, .tar.gz asset URL).
fn latest_release() -> (String, String) {
    let out = Command::new("curl")
        .args(["-fsSL", &format!("https://api.github.com/repos/{REPO}/releases/latest")])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .unwrap_or_else(|| fail(&t("GitHub nicht erreichbar.", "GitHub not reachable.")));
    let json: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap_or_else(|_| fail("GitHub: invalid JSON"));
    let tag = json["tag_name"].as_str().unwrap_or_else(|| fail("GitHub: no tag_name")).to_string();
    let url = json["assets"]
        .as_array()
        .and_then(|a| a.iter().find(|a| a["name"].as_str().is_some_and(|n| n.ends_with(".tar.gz"))))
        .and_then(|a| a["browser_download_url"].as_str())
        .unwrap_or_else(|| fail(&format!("{tag}: no .tar.gz asset")))
        .to_string();
    (tag, url)
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

    if env::args().nth(1).as_deref() == Some("--folder") {
        let _ = Command::new("xdg-open").arg(&dest).status();
        return;
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

    let (tag, url) = latest_release();
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
