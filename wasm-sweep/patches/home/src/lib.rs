use std::path::PathBuf;

pub fn home_dir() -> Option<PathBuf> {
    #[cfg(not(target_arch = "wasm32"))]
    {
        std::env::var_os("HOME").map(PathBuf::from)
    }
    #[cfg(target_arch = "wasm32")]
    {
        None
    }
}

pub fn cargo_home() -> std::io::Result<PathBuf> {
    home_dir()
        .map(|p| p.join(".cargo"))
        .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::NotFound, "no home dir"))
}
