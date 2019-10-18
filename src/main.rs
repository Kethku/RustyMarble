use std::path::PathBuf;
use std::process::Command;
use std::str;

use image::RgbImage;
use ndarray::{Array2, Ix2, Array3, Axis, stack};
use netcdf;

fn ls(path: &str) -> Vec<String> {
    let dir_path = [ path, "/" ].concat();
    let result = Command::new("aws")
        .args(&["s3", "ls", &dir_path, "--no-sign-request"])
        .output()
        .expect("aws ls failed.");

    let output = str::from_utf8(&result.stdout).expect("output didn't parse");

    output
        .lines()
        .map(|line| line.to_string()
                        .trim()[4..]
                        .trim_end_matches("/")
                        .to_string())
        .collect()
}

fn download(object_path: &str, output_path: &str) {
    let result = Command::new("aws")
        .args(&["s3", "cp", &["s3://", object_path].concat(), output_path, "--no-sign-request" ])
        .output().expect("aws download failed");

    dbg!(str::from_utf8(&result.stdout).expect("output didn't parse").to_string());
}

fn append_folder(path: &mut String, folder_name: String) {
    path.push_str(&["/", &folder_name].concat())
}

fn biggest_folder(path: &str) -> String {
    ls(path)
        .iter()
        .map(|folder| folder.parse::<i32>().expect("folder does not have number name"))
        .max().expect("no folders available")
        .to_string()
}

fn newest_output_folder(product: &str) -> String {
    let mut path = product.to_string();

    let year = biggest_folder(&path);
    println!("{}", year);
    append_folder(&mut path, year);
    let day = biggest_folder(&path);
    println!("{}", day);
    append_folder(&mut path, day);
    let hour = biggest_folder(&path);
    println!("{}", hour);
    append_folder(&mut path, format!("{:0>2}", hour));

    path
}

fn most_recent_file_path(product: &str) -> String {
    let mut newest_output = newest_output_folder(product);
    let files = ls(&newest_output);
    let most_recent = files.last().expect("no files available");

    let info_parts = most_recent.split_whitespace();

    append_folder(&mut newest_output, info_parts.last().expect("No split parts").to_string());

    newest_output
}

fn extract_channel(file: &netcdf::File, channel_name: &str) -> Array2::<i16> {
    file.root()
        .variables()[channel_name]
        .values::<i16>(None, None).expect("Data not made of shorts")
        .into_dimensionality::<Ix2>().expect("Data is not square")
}

fn reproject(channel: &Array2::<i16>) -> Array2::<f32> {
    dbg!(channel.iter().max());
    let gamma = 2.2;
    let exponent = 1.0 / gamma;
    let result = channel.map(|value| ((*value + 1) as f32 / 4096.0).powf(exponent));
    result
}

fn prepare_channel(channel: Array2::<f32>) -> Array3::<u8> {
    channel.insert_axis(Axis(2)).map(|value| (*value * 255.0).floor() as u8)
}

fn data_to_image(arr: Array3<u8>) -> RgbImage {
    assert!(arr.is_standard_layout());

    let (height, width, _) = arr.dim();
    let raw = arr.into_raw_vec();

    RgbImage::from_raw(width as u32, height as u32, raw)
        .expect("container should have the right size for the image dimensions")
}

fn build_truecolor_image(data_path: PathBuf, relative_out: &str) {
    let file = netcdf::File::open(&data_path).expect("Could not open file");
    let r = reproject(&extract_channel(&file, "CMI_C02"));
    println!("reprojected r");
    let g = reproject(&extract_channel(&file, "CMI_C03"));
    println!("reprojected g");
    let b = reproject(&extract_channel(&file, "CMI_C01"));
    println!("reprojected b");
    let corrected_g = 0.45 * r.clone() + 0.1 * g + 0.45 * b.clone();
    println!("corrected g");

    let image_data = stack(Axis(2), &[prepare_channel(r).view(), prepare_channel(corrected_g).view(), prepare_channel(b).view()]).expect("Could not build stack");
    println!("stacked channels");
    let image = data_to_image(image_data);
    image.save(relative_out).expect("Could not save image");
}

fn main() {
    download(&most_recent_file_path("noaa-goes16/ABI-L2-MCMIPF"), "../output/current.nc");
    let mut output_path = PathBuf::new();
    output_path.push("~");
    output_path.push("output");
    output_path.push("current.nc");
    build_truecolor_image(output_path, "~/output/current.jpg");
}
