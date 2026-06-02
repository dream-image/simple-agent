use std::path::PathBuf;

pub fn  getCurrentWorkPath() ->PathBuf{
     PathBuf::from(env!("CARGO_MANIFEST_DIR")).parent().unwrap().to_path_buf()
}