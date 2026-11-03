use image::GenericImageView;
use std::io::Write;

/*
 * convert .png to custom binary format
 * $cargo run -bin image_convert
 * - generate .rgba files in /resource
 */
pub fn main() {
    std::env::set_var("RUST_LOG", "info");
    env_logger::init();

    // hard code temp
    let dir_name_hard_code = "write here";
    let files = get_dir_files(&dir_name_hard_code).expect("Failed to read image folder path");

    log::info!("found {} files", files.len());

    for file in files {
        let file_name = file.file_name().unwrap().to_str().unwrap();
        log::info!("{}", &file_name);

        convert_and_save_rgba_file(&file);

        log::info!(
            "saved {} .rgba format",
            file.file_name().unwrap().to_str().unwrap()
        );
    }
}

pub fn get_dir_files(dir_name: &str) -> anyhow::Result<Vec<std::path::PathBuf>> {
    let dir_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("resource")
        .join(dir_name);

    let dir = std::fs::read_dir(dir_path)?;

    let mut files: Vec<std::path::PathBuf> = Vec::new();
    for item in dir.into_iter() {
        files.push(item?.path());
    }
    Ok(files)
}

pub fn convert_and_save_rgba_file(file: &std::path::PathBuf) {
    let extension = file
        .extension()
        .expect("Should have extension")
        .to_str()
        .expect("Should be str");
    if !extension.contains("png") && !extension.contains("jpg") && !extension.contains("jpeg") {
        return;
    }

    let binary_data = std::fs::read(&file).unwrap();
    let image = image::load_from_memory(&binary_data).unwrap();

    // 4byte
    let image_width = image.dimensions().0;
    let mut width_u8 = image_width.to_be_bytes().to_vec();
    // 4byte
    let image_height = image.dimensions().1;
    let mut height_u8 = image_height.to_be_bytes().to_vec();
    // else
    let mut rgba_binary = image.to_rgba8().to_vec();

    log::info!(
        "width : {}, height : {}, size : {} byte",
        image_width,
        image_height,
        rgba_binary.len()
    );

    let mut out_path = file.clone();
    out_path.set_extension("rgba");

    let mut out_binary: Vec<u8> = Vec::new();
    out_binary.append(&mut width_u8);
    out_binary.append(&mut height_u8);
    out_binary.append(&mut rgba_binary);

    let mut file = std::fs::File::create(&out_path).unwrap();
    file.write_all(&out_binary).unwrap();
    file.flush().unwrap();
}

pub fn u32_to_u8_vec(u32: u32) -> Vec<u8> {
    let a = ((u32 >> 24) & 0xff) as u8;
    let b = ((u32 >> 16) & 0xff) as u8;
    let c = ((u32 >> 8) & 0xff) as u8;
    let d = ((u32 >> 0) & 0xff) as u8;

    let mut out = Vec::new();
    out.push(a);
    out.push(b);
    out.push(c);
    out.push(d);

    return out;
}
