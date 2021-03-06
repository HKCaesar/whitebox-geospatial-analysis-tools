extern crate whitebox_tools;
extern crate time;
extern crate num_cpus;

use std::io;
use std::env;
use std::path;
use std::f64;
use std::i64;
use std::sync::Arc;
use std::sync::mpsc;
use std::thread;
use whitebox_tools::raster::*;
use whitebox_tools::structures::array2d::Array2D;

const TOOL_NAME: &str = "elev_percentile";

fn main() {
    let sep: String = path::MAIN_SEPARATOR.to_string();
    let mut input_file = String::new();
    let mut output_file = String::new();
    let mut working_directory: String = "".to_string();
    let mut filter_size_x = 11usize;
    let mut filter_size_y = 11usize;
    let mut verbose: bool = false;
    let mut keyval: bool;
    let args: Vec<String> = env::args().collect();
    if args.len() <= 1 { panic!("Tool run with no paramters. Please see help (-h) for parameter descriptions."); }
    for i in 0..args.len() {
        let mut arg = args[i].replace("\"", "");
        arg = arg.replace("\'", "");
        let cmd = arg.split("="); // in case an equals sign was used
        let vec = cmd.collect::<Vec<&str>>();
        keyval = false;
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
        } else if vec[0].to_lowercase() == "-wd" || vec[0].to_lowercase() == "--wd" {
            if keyval {
                working_directory = vec[1].to_string();
            } else {
                working_directory = args[i+1].to_string();
            }
        } else if vec[0].to_lowercase() == "-filter" || vec[0].to_lowercase() == "--filter" {
            if keyval {
                filter_size_x = vec[1].to_string().parse::<usize>().unwrap();
            } else {
                filter_size_x = args[i+1].to_string().parse::<usize>().unwrap();
            }
            filter_size_y = filter_size_x;
        } else if vec[0].to_lowercase() == "-filterx" || vec[0].to_lowercase() == "--filterx" {
            if keyval {
                filter_size_x = vec[1].to_string().parse::<usize>().unwrap();
            } else {
                filter_size_x = args[i+1].to_string().parse::<usize>().unwrap();
            }
        } else if vec[0].to_lowercase() == "-filtery" || vec[0].to_lowercase() == "--filtery" {
            if keyval {
                filter_size_y = vec[1].to_string().parse::<usize>().unwrap();
            } else {
                filter_size_y = args[i+1].to_string().parse::<usize>().unwrap();
            }
        } else if vec[0].to_lowercase() == "-v" || vec[0].to_lowercase() == "--verbose" {
            verbose = true;
        } else if vec[0].to_lowercase() == "-h" || vec[0].to_lowercase() == "--help" ||
            vec[0].to_lowercase() == "--h"{
            let mut s: String = "Help:\n".to_owned();
                     s.push_str("-i, --input   Input raster DEM file.\n");
                     s.push_str("-o, --output  Output raster file.\n");
                     s.push_str("-wd, --wd     Optional working directory. If specified, filenames parameters need not include a full path.\n");
                     s.push_str("--filter      Size of the filter kernel (default is 11).\n");
                     s.push_str("--filterx     Optional size of the filter kernel in the x-direction (default is 11; not used if --filter is specified).\n");
                     s.push_str("--filtery     Optional size of the filter kernel in the y-direction (default is 11; not used if --filter is specified).\n");
                     s.push_str("-version      Prints the tool version number.\n");
                     s.push_str("-h            Prints help information.\n\n");
                     s.push_str("Example usage:\n\n");
                     s.push_str(&format!(">> .*{} -wd *path*to*data* -i=input.dep -o=output.dep --filter=25\n", TOOL_NAME).replace("*", &sep));
            println!("{}", s);
            return;
        } else if vec[0].to_lowercase() == "-version" || vec[0].to_lowercase() == "--version" {
            const VERSION: Option<&'static str> = option_env!("CARGO_PKG_VERSION");
            println!("{} v{}", TOOL_NAME, VERSION.unwrap_or("unknown"));
            return;
        }
    }

    match run(input_file, output_file, working_directory,
        filter_size_x, filter_size_y, verbose) {
        Ok(()) => println!("Complete!"),
        Err(err) => panic!("{}", err),
    }
}

fn run(mut input_file: String, mut output_file: String, mut working_directory: String,
    mut filter_size_x: usize, mut filter_size_y: usize, verbose: bool) -> Result<(), io::Error> {

    if verbose {
        println!("***************{}", "*".repeat(TOOL_NAME.len()));
        println!("* Welcome to {} *", TOOL_NAME);
        println!("***************{}", "*".repeat(TOOL_NAME.len()));
    }

    let sep: String = path::MAIN_SEPARATOR.to_string();

    if filter_size_x < 3 { filter_size_x = 3; }
    if filter_size_y < 3 { filter_size_y = 3; }

	// The filter dimensions must be odd numbers such that there is a middle pixel
    if (filter_size_x as f64 / 2f64).floor() == (filter_size_x as f64 / 2f64) {
        filter_size_x += 1;
    }
    if (filter_size_y as f64 / 2f64).floor() == (filter_size_y as f64 / 2f64) {
        filter_size_y += 1;
    }

    // let (mut z, mut z_n): (f64, f64);
    let midpoint_x = (filter_size_x as f64 / 2f64).floor() as isize;
    let midpoint_y = (filter_size_y as f64 / 2f64).floor() as isize;
    let mut progress: usize;
    let mut old_progress: usize = 1;

    if !working_directory.ends_with(&sep) {
        working_directory.push_str(&(sep.to_string()));
    }

    if !input_file.contains(&sep) {
        input_file = format!("{}{}", working_directory, input_file);
    }
    if !output_file.contains(&sep) {
        output_file = format!("{}{}", working_directory, output_file);
    }

    if verbose { println!("Reading data...") };

    let input = Arc::new(Raster::new(&input_file, "r")?);
    // let input = Raster::new(&input_file, "r")?;

    let start = time::now();

    // first bin the data
    let rows = input.configs.rows as isize;
    let columns = input.configs.columns as isize;
    let num_sig_digits = 2;
    let multiplier = 10f64.powi(num_sig_digits);
    let min_val = input.configs.minimum;
    let max_val = input.configs.maximum;
	let min_bin = (min_val * multiplier).floor() as i64;
	let num_bins = (max_val * multiplier).floor() as i64 - min_bin + 1;
    let bin_nodata = i64::MIN;
    let mut binned_data : Array2D<i64> = Array2D::new(rows, columns, bin_nodata, bin_nodata)?;

    let num_procs = num_cpus::get() as isize;
    let row_block_size = rows / num_procs;
    let (tx, rx) = mpsc::channel();

    let mut starting_row;
    let mut ending_row = 0;
    let mut id = 0;
    while ending_row < rows {
        let input = input.clone();
        let rows = rows.clone();
        starting_row = id * row_block_size;
        ending_row = starting_row + row_block_size;
		if ending_row > rows {
			ending_row = rows;
		}
        id += 1;
        let tx1 = tx.clone();
        thread::spawn(move || {
            let nodata = input.configs.nodata;
            let columns = input.configs.columns as isize;
            let mut z : f64;
            let mut val : i64;
            for row in starting_row..ending_row {
                let mut data = vec![bin_nodata; columns as usize];
                for col in 0..columns {
                    z = input.get_value(row, col);
                    if z != nodata {
                        val = (z*multiplier).floor() as i64 - min_bin;
                        data[col as usize] = val;
                    }
                }
                tx1.send((row, data)).unwrap();
            }
        });
    }

    for row in 0..rows {
        let data = rx.recv().unwrap();
        binned_data.set_row_data(data.0, data.1);
        if verbose {
            progress = (100.0_f64 * row as f64 / (rows - 1) as f64) as usize;
            if progress != old_progress {
                println!("Binning data: {}%", progress);
                old_progress = progress;
            }
        }
    }

    let bd = Arc::new(binned_data); // wrap binned_data in an Arc
    let mut output = Raster::initialize_using_file(&output_file, &input);
    // let mut starting_row;
    ending_row = 0;
    let (tx, rx) = mpsc::channel();
    let mut id = 0;
    while ending_row < rows {
        let input = input.clone();
        let binned_data = bd.clone();
        let rows = rows.clone();
        starting_row = id * row_block_size;
        ending_row = starting_row + row_block_size;
		if ending_row > rows {
			ending_row = rows;
		}
        id += 1;
        let tx1 = tx.clone();
        thread::spawn(move || {
            let nodata = input.configs.nodata;
            let columns = input.configs.columns as isize;
            let (mut bin_val, mut bin_val_n, mut old_bin_val) : (i64, i64, i64);
			let (mut start_col, mut end_col, mut start_row, mut end_row): (isize, isize, isize, isize);
            let mut m : i64;
			let (mut n, mut n_less_than): (f64, f64);
            for row in starting_row..ending_row {
                start_row = row - midpoint_y;
                end_row = row + midpoint_y;
                let mut histo : Vec<i64> = vec![];
                old_bin_val = bin_nodata;
				n = 0.0;
				n_less_than = 0.0;
                let mut data = vec![nodata; columns as usize];
                for col in 0..columns {
                    bin_val = binned_data.get_value(row, col);
                    if bin_val != bin_nodata {
                        if old_bin_val != bin_nodata {
                            // remove the trailing column from the histo
                            for row2 in start_row..end_row+1 {
								bin_val_n = binned_data.get_value(row2, col-midpoint_x-1);
								if bin_val_n != bin_nodata {
									histo[bin_val_n as usize] -= 1;
									n -= 1.0;
									if bin_val_n < old_bin_val {
										n_less_than -= 1.0;
									}
								}
							}

                            // add the leading column to the histo
							for row2 in start_row..end_row+1 {
								bin_val_n = binned_data.get_value(row2, col+midpoint_x);
								if bin_val_n != bin_nodata {
									histo[bin_val_n  as usize] += 1;
									n += 1.0;
									if bin_val_n < old_bin_val {
										n_less_than += 1.0;
									}
								}
							}

                            // how many cells lie between the bins of binVal and oldBinVal?
							if old_bin_val < bin_val {
								m = 0;
                                for v in old_bin_val..bin_val {
									m += histo[v as usize];
								}
								n_less_than += m as f64;
							} else if old_bin_val > bin_val {
								m = 0;
                                for v in bin_val..old_bin_val {
									m += histo[v as usize];
								}
								n_less_than -= m as f64;
							} // otherwise they are in the same bin and there is no need to update

                        } else {
                            // initialize the histogram
							histo = vec![0i64; num_bins as usize];
							n = 0.0;
							n_less_than = 0.0;
                            start_col = col - midpoint_x;
                            end_col = col + midpoint_x;
							for col2 in start_col..end_col+1 {
								for row2 in start_row..end_row+1 {
									bin_val_n = binned_data.get_value(row2, col2);
									if bin_val_n != bin_nodata {
										histo[bin_val_n as usize] += 1;
                                        n += 1f64;
										if bin_val_n < bin_val {
											n_less_than += 1f64;
										}
									}
								}
							}
                        }
                    }

                    if n > 0f64 {
						data[col as usize] = n_less_than / n * 100.0;
					} else {
						data[col as usize] = nodata;
					}

                    old_bin_val = bin_val;
                }
                tx1.send((row, data)).unwrap();
            }
        });
    }

    for row in 0..rows {
        let data = rx.recv().unwrap();
        output.set_row_data(data.0, data.1);
        if verbose {
            progress = (100.0_f64 * row as f64 / (rows - 1) as f64) as usize;
            if progress != old_progress {
                println!("Performing analysis: {}%", progress);
                old_progress = progress;
            }
        }
    }

    let end = time::now();
    let elapsed_time = end - start;
    output.configs.display_min = 0.0;
    output.configs.display_max = 100.0;
    output.configs.palette = "blue_white_red.plt".to_string();
    output.add_metadata_entry(format!("Created by whitebox_tools\' {} tool", TOOL_NAME));
    output.add_metadata_entry(format!("Input file: {}", input_file));
    output.add_metadata_entry(format!("Filter size x: {}", filter_size_x));
    output.add_metadata_entry(format!("Filter size y: {}", filter_size_y));
    output.add_metadata_entry(format!("Elapsed Time (excluding I/O): {}", elapsed_time).replace("PT", ""));

    if verbose { println!("Saving data...") };
    let _ = match output.write() {
        Ok(_) => if verbose { println!("Output file written") },
        Err(e) => return Err(e),
    };

    println!("{}", &format!("Elapsed Time (excluding I/O): {}", elapsed_time).replace("PT", ""));

    Ok(())
}
