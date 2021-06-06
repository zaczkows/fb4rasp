pub fn get_cpu_temperature() -> f32 {
    use std::io::prelude::*;

    let filename = "/sys/class/thermal/thermal_zone0/temp";
    let file = std::fs::File::open(filename);
    if let Ok(mut file) = file {
        let mut contents = String::new();
        if file.read_to_string(&mut contents).is_ok() {
            let contents = contents.trim();
            return contents.parse::<f32>().unwrap() / 1000.0;
        }
    }
    log::warn!("Failure to get CPU temperature from {}", filename);
    0.0
}
