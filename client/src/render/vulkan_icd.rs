//! En Linux el loader de Vulkan abre todos los ICD de `icd.d`. Un JSON de
//! NVIDIA que apunta a `libGLX_nvidia.so.0` sin entrada Vulkan hace que wgpu
//! loguee dos ERROR aunque la GPU sea AMD. Acá se dejan solo los manifest
//! cuyo `.so` exporta `vkCreateInstance`.

use std::ffi::{c_char, c_void};
use std::path::{Path, PathBuf};

pub fn keep_only_working_drivers() {
    if std::env::var_os("VK_DRIVER_FILES").is_some()
        || std::env::var_os("VK_ICD_FILENAMES").is_some()
    {
        return;
    }

    let working = working_manifests();
    if working.is_empty() {
        return;
    }

    let joined = working
        .iter()
        .map(|path| path.display().to_string())
        .collect::<Vec<_>>()
        .join(":");
    // El loader todavía no está cargado: hay que fijar esto antes de wgpu.
    // SAFETY: un solo hilo, al arranque, antes de crear el `Instance`.
    unsafe { std::env::set_var("VK_DRIVER_FILES", &joined) };
    tracing::debug!(drivers = %joined, "ICD Vulkan usables");
}

fn working_manifests() -> Vec<PathBuf> {
    let mut found = Vec::new();
    for dir in icd_dirs() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|ext| ext.to_str()) != Some("json") {
                continue;
            }
            match manifest_library(&path) {
                Some(library) if library_exports_vk_create_instance(&library) => found.push(path),
                Some(library) => {
                    tracing::debug!(
                        icd = %path.display(),
                        library = %library,
                        "ICD Vulkan omitido"
                    );
                }
                None => {}
            }
        }
    }
    found.sort();
    found.dedup();
    found
}

fn icd_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    if let Some(xdg_home) = std::env::var_os("XDG_DATA_HOME") {
        dirs.push(PathBuf::from(xdg_home).join("vulkan/icd.d"));
    } else if let Some(home) = std::env::var_os("HOME") {
        dirs.push(PathBuf::from(home).join(".local/share/vulkan/icd.d"));
    }

    let data_dirs = std::env::var("XDG_DATA_DIRS")
        .unwrap_or_else(|_| "/usr/local/share:/usr/share".to_string());
    for dir in data_dirs.split(':').filter(|dir| !dir.is_empty()) {
        dirs.push(PathBuf::from(dir).join("vulkan/icd.d"));
    }
    dirs.push(PathBuf::from("/etc/vulkan/icd.d"));
    dirs
}

fn manifest_library(path: &Path) -> Option<String> {
    let text = std::fs::read_to_string(path).ok()?;
    let key = "\"library_path\"";
    let rest = text.get(text.find(key)? + key.len()..)?;
    let start = rest.find('"')? + 1;
    let end = start + rest[start..].find('"')?;
    Some(rest[start..end].to_string())
}

fn library_exports_vk_create_instance(library_path: &str) -> bool {
    type GetInstanceProcAddr = unsafe extern "C" fn(*mut c_void, *const c_char) -> *const c_void;

    unsafe {
        let Ok(library) = libloading::Library::new(library_path) else {
            return false;
        };
        let Ok(get_proc) = library.get::<GetInstanceProcAddr>(b"vk_icdGetInstanceProcAddr") else {
            return false;
        };
        !get_proc(std::ptr::null_mut(), c"vkCreateInstance".as_ptr()).is_null()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_library_path() {
        let dir = std::env::temp_dir().join("revvy-icd-test");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("sample.json");
        std::fs::write(
            &path,
            r#"{ "ICD": { "library_path": "libvulkan_radeon.so" } }"#,
        )
        .unwrap();
        assert_eq!(
            manifest_library(&path).as_deref(),
            Some("libvulkan_radeon.so")
        );
    }
}
