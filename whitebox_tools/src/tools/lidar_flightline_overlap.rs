extern crate time;

use std::f64;
use std::io::{Error, ErrorKind};
use std::path;
use lidar::las;
use raster::*;
use structures::fixed_radius_search::FixedRadiusSearch;

pub fn get_tool_name() -> String {
    return "lidar_flightline_overlap".to_string();
}

pub fn get_tool_description() -> String {
    let s = "Reads a LiDAR (LAS) point file and outputs a raster containing the number of overlapping
flight lines in each grid cell.";

    return s.to_string();
}

pub fn get_tool_parameters() -> String {
    let s = "-i, --input        Input LAS file.
-o, --output       Output raster file.
--resolution       Output raster's grid resolution.
--palette          Optional palette name (for use with Whitebox raster files).";
    return s.to_string();
}

pub fn get_example_usage() -> Option<String> {
    let s = "./whitebox-tools -r=lidar_flightline_overlap --wd=\"/dir/to/data\" --args=\"-i=file.las -o=outfile.dep --resolution=2.0\"
./whitebox-tools -r=lidar_flightline_overlap --wd=\"/dir/to/data\" --args=\"-i=file.las -o=outfile.dep --resolution=5.0 --palette=light_quant.plt\"";
    return Some(s.to_string());
}

pub fn run<'a>(args: Vec<String>, working_directory: &'a str, verbose: bool) -> Result<(), Error> {
    let mut input_file: String = "".to_string();
    let mut output_file: String = "".to_string();
    let mut grid_res: f64 = 1.0;
    let mut palette = "default".to_string();

    // read the arguments
    if args.len() == 0 {
        return Err(Error::new(ErrorKind::InvalidInput, "Tool run with no paramters. Please see help (-h) for parameter descriptions."));
    }
    for i in 0..args.len() {
        let mut arg = args[i].replace("\"", "");
        arg = arg.replace("\'", "");
        let cmd = arg.split("="); // in case an equals sign was used
        let vec = cmd.collect::<Vec<&str>>();
        let mut keyval = false;
        if vec.len() > 1 { keyval = true; }
        if vec[0].to_lowercase() == "-i" || vec[0].to_lowercase() == "--input" {
            if keyval {
                input_file = vec[1].to_string();
            } else {
                input_file = args[i+1].to_string();
            }
        } else if vec[0].to_lowercase() == "-o" || vec[0].to_lowercase() == "--output" {
            if keyval {
                output_file = vec[1].to_string();
            } else {
                output_file = args[i+1].to_string();
            }
        } else if vec[0].to_lowercase() == "-resolution" || vec[0].to_lowercase() == "--resolution" {
            if keyval {
                grid_res = vec[1].to_string().parse::<f64>().unwrap();
            } else {
                grid_res = args[i+1].to_string().parse::<f64>().unwrap();
            }
        } else if vec[0].to_lowercase() == "-palette" || vec[0].to_lowercase() == "--palette" {
            if keyval {
                palette = vec[1].to_string();
            } else {
                palette = args[i+1].to_string();
            }
        }
    }

    if !input_file.contains(path::MAIN_SEPARATOR) {
        input_file = format!("{}{}", working_directory, input_file);
    }
    if !output_file.contains(path::MAIN_SEPARATOR) {
        output_file = format!("{}{}", working_directory, output_file);
    }

    if verbose {
        println!("***************************************");
        println!("* Welcome to lidar_flightline_overlap *");
        println!("***************************************");
    }

    let start = time::now();

    if verbose { println!("Reading input LAS file..."); }
    let input = match las::LasFile::new(&input_file, "r") {
        Ok(lf) => lf,
        Err(_) => return Err(Error::new(ErrorKind::NotFound, format!("No such file or directory ({})", input_file))),
    };

    // Make sure that the input LAS file have GPS time data?
    if input.header.point_format == 0u8 || input.header.point_format == 2u8 {
        panic!("The input file has a Point Format that does not include GPS time, which is required for the operation of this tool.");
    }

    let n_points = input.header.number_of_points as usize;
    let num_points: f64 = (input.header.number_of_points - 1) as f64; // used for progress calculation only

    if verbose { println!("Performing analysis..."); }
    // let search_dist = grid_res / 2.0;
    let mut frs: FixedRadiusSearch<usize> = FixedRadiusSearch::new(grid_res);
    let mut gps_times = vec![-1f64; n_points];
    let (mut x, mut y, mut gps_time) : (f64, f64, f64);
    let mut progress: usize;
    let mut old_progress: usize = 1;
    for i in 0..n_points {
        match input.get_record(i) {
            las::LidarPointRecord::PointRecord1 { point_data, gps_data } => {
                x = point_data.x;
                y = point_data.y;
                gps_time = gps_data;
            },
            las::LidarPointRecord::PointRecord3 { point_data, gps_data, rgb_data } => {
                x = point_data.x;
                y = point_data.y;
                gps_time = gps_data;
                let _ = rgb_data; // just to kill the 'unused variable' warning
            },
            _ => {
                panic!("The input file has a Point Format that does not include GPS time, which is required for the operation of this tool.");
            }
        };
        frs.insert(x, y, i);
        gps_times[i] = gps_time;
        if verbose {
            progress = (100.0_f64 * i as f64 / num_points) as usize;
            if progress != old_progress {
                println!("Binning points: {}%", progress);
                old_progress = progress;
            }
        }
    }

    let west: f64 = input.header.min_x; // - 0.5 * grid_res;
	let north: f64 = input.header.max_y; // + 0.5 * grid_res;
	let rows: usize = (((north - input.header.min_y) / grid_res).ceil()) as usize;
	let columns: usize = (((input.header.max_x - west) / grid_res).ceil()) as usize;
	let south: f64 = north - rows as f64 * grid_res;
	let east = west + columns as f64 * grid_res;
    let nodata = -32768.0f64;

    let mut configs = RasterConfigs{..Default::default()};
    configs.rows = rows;
    configs.columns = columns;
    configs.north = north;
    configs.south = south;
    configs.east = east;
    configs.west = west;
    configs.resolution_x = grid_res;
    configs.resolution_y = grid_res;
    configs.nodata = nodata;
    configs.data_type = DataType::F64;
    configs.photometric_interp = PhotometricInterpretation::Continuous;
    configs.palette = palette;
    // configs.projection = input.configs.projection.clone();
    // configs.xy_units = input.configs.xy_units.clone();
    // configs.z_units = input.configs.z_units.clone();
    // configs.endian = input.configs.endian.clone();
    // configs.epsg_code = input.configs.epsg_code;
    // configs.coordinate_ref_system_wkt = input.configs.coordinate_ref_system_wkt.clone();
    let mut output = Raster::initialize_using_config(&output_file, &configs);
    let time_threshold = 15f64;
    let (mut x_n, mut y_n): (f64, f64);
    let mut index_n: usize;
    let half_res_sqrd = grid_res / 2.0 * grid_res / 2.0;
    for row in 0..rows as isize {
        for col in 0..columns as isize {
            x = west + col as f64 * grid_res + 0.5;
            y = north - row as f64 * grid_res - 0.5;
            let ret = frs.search(x, y);
            if ret.len() > 0 {
                let mut times = vec![];
                for j in 0..ret.len() {
                    index_n = ret[j].0;
                    let p = input[index_n];
                    x_n = p.x;
                    y_n = p.y;
                    if (x_n - x) * (x_n - x) <= half_res_sqrd && (y_n - y) * (y_n - y) <= half_res_sqrd { // it falls within the grid cell
                        times.push(gps_times[ret[j].0]);
                    }
                }
                if times.len() > 0 {
                    times.sort_by(|a, b| a.partial_cmp(&b).unwrap());
                    let mut num_flightlines = 1.0;
                    for j in 1..times.len() {
                        if times[j] - times[j-1] > time_threshold {
                            num_flightlines += 1.0;
                        }
                    }
                    output.set_value(row, col, num_flightlines);
                } else {
                    output.set_value(row, col, nodata);
                }
            } else {
                output.set_value(row, col, nodata);
            }
        }
        if verbose {
            progress = (100.0_f64 * row as f64 / (rows - 1) as f64) as usize;
            if progress != old_progress {
                println!("Progress: {}%", progress);
                old_progress = progress;
            }
        }
    }

    let end = time::now();
    let elapsed_time = end - start;
    output.add_metadata_entry("Created by whitebox_tools\' lidar_flightline_overlap tool".to_owned());
    output.add_metadata_entry(format!("Input file: {}", input_file));
    output.add_metadata_entry(format!("Elapsed Time (excluding I/O): {}", elapsed_time).replace("PT", ""));

    if verbose { println!("Saving data...") };
    let _ = match output.write() {
        Ok(_) => if verbose { println!("Output file written") },
        Err(e) => return Err(e),
    };

    Ok(())
}
