//! List device sample-rate ranges without opening a microphone or speaker stream.
use cpal::traits::{DeviceTrait, HostTrait};

fn main() -> eyre::Result<()> {
    let host = cpal::default_host();
    for (input, label) in [(true, "INPUT"), (false, "OUTPUT")] {
        let devices = if input {
            host.input_devices()?
        } else {
            host.output_devices()?
        };
        for device in devices {
            println!("{label}: {}", device.name()?);
            let ranges: Vec<_> = if input {
                device.supported_input_configs()?.collect()
            } else {
                device.supported_output_configs()?.collect()
            };
            for range in ranges {
                println!(
                    "  {} channels {:?} {}–{} Hz",
                    range.channels(),
                    range.sample_format(),
                    range.min_sample_rate().0,
                    range.max_sample_rate().0
                );
            }
        }
    }
    Ok(())
}
