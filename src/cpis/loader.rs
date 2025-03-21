use dashmap::DashMap;
use rayon::prelude::*;

pub fn load_cpis() -> DashMap<String, String> {
    let cpi_dir = "./CPIs";

    let entries = match std::fs::read_dir(cpi_dir) {
        Ok(iter) => iter,
        Err(_) => return DashMap::new(),
    };

    let paths: Vec<_> = entries
        .filter_map(|entry| entry.ok().map(|e| e.path()))
        .filter(|path| path.is_file())
        .collect();

    let cpi_map = DashMap::new();

    paths.par_iter().for_each(|path| {
        if let (Some(file_name), Ok(content)) = (
            path.file_name()
                .and_then(|s| s.to_str())
                .map(String::from),
            std::fs::read_to_string(path),
        ) {
            cpi_map.insert(file_name, content);
        }
    });

    cpi_map
}
