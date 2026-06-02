use std::path::PathBuf;

pub fn get_current_work_path() ->PathBuf{
     PathBuf::from(env!("CARGO_MANIFEST_DIR")).parent().unwrap().to_path_buf()
}