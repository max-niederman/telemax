use serde::Serialize;
use std::cell::RefCell;
use std::ops::Deref;
use std::rc::Rc;

use libpulse_binding as pulse;
use pulse::callbacks::ListResult;
use pulse::context::{Context, FlagSet as ContextFlagSet};
use pulse::def::BufferAttr;
use pulse::mainloop::standard::{IterateResult, Mainloop};
use pulse::sample::{Format, Spec};
use pulse::stream::{FlagSet as StreamFlagSet, PeekResult, Stream};

use rustfft::num_complex::Complex;
use rustfft::FftPlanner;

use tokio::sync::broadcast;

const SAMPLE_RATE: u32 = 48000;
const FFT_SIZE: usize = 2048;
const NUM_BANDS: usize = 9;

/// Frequency band edges in Hz. 9 bands between 10 consecutive edges.
const BAND_EDGES: [f32; NUM_BANDS + 1] = [
    30.0, 60.0, 150.0, 400.0, 1000.0, 2500.0, 6000.0, 12000.0, 16000.0, 20000.0,
];

/// Smoothing factor for exponential moving average (0.0 = no update, 1.0 = no smoothing).
const SMOOTH_ALPHA: f32 = 0.3;

#[derive(Debug, Clone, Serialize)]
pub struct AudioLevel {
    pub bands: Vec<f32>,
}

/// Spawns a dedicated thread that monitors audio levels from the default sink's
/// monitor source using FFT-based frequency analysis.
///
/// Sends approximately 30 `AudioLevel` messages per second on the broadcast channel.
/// If PulseAudio connection fails, logs the error and returns without panicking.
pub fn start_monitoring(tx: broadcast::Sender<AudioLevel>) {
    std::thread::spawn(move || {
        if let Err(e) = run_pa_monitor(tx) {
            tracing::error!("PulseAudio monitoring failed: {}", e);
        }
    });
}

fn run_pa_monitor(tx: broadcast::Sender<AudioLevel>) -> Result<(), String> {
    let mainloop = Rc::new(RefCell::new(
        Mainloop::new().ok_or("Failed to create PulseAudio mainloop")?,
    ));

    let context = Rc::new(RefCell::new(
        Context::new(mainloop.borrow().deref(), "telemax-audio")
            .ok_or("Failed to create PulseAudio context")?,
    ));

    context
        .borrow_mut()
        .connect(None, ContextFlagSet::NOFLAGS, None)
        .map_err(|e| format!("Failed to connect PulseAudio context: {:?}", e))?;

    // Wait for context to become ready.
    loop {
        match mainloop.borrow_mut().iterate(true) {
            IterateResult::Quit(_) | IterateResult::Err(_) => {
                return Err(
                    "PulseAudio mainloop iterate failed while waiting for context".into(),
                );
            }
            IterateResult::Success(_) => {}
        }
        match context.borrow().get_state() {
            pulse::context::State::Ready => break,
            pulse::context::State::Failed | pulse::context::State::Terminated => {
                return Err("PulseAudio context failed or terminated".into());
            }
            _ => {}
        }
    }

    // Query the default sink name via server info, then get its monitor source.
    let monitor_source: Rc<RefCell<Option<String>>> = Rc::new(RefCell::new(None));

    // Step 1: Get default sink name from server info.
    let default_sink_name: Rc<RefCell<Option<String>>> = Rc::new(RefCell::new(None));
    {
        let sink_name_ref = Rc::clone(&default_sink_name);
        let _op = context.borrow().introspect().get_server_info(move |info| {
            if let Some(ref name) = info.default_sink_name {
                *sink_name_ref.borrow_mut() = Some(name.to_string());
            }
        });

        // Iterate until we get the server info callback.
        loop {
            match mainloop.borrow_mut().iterate(true) {
                IterateResult::Quit(_) | IterateResult::Err(_) => {
                    return Err("Mainloop iterate failed while getting server info".into());
                }
                IterateResult::Success(_) => {}
            }
            if default_sink_name.borrow().is_some() {
                break;
            }
        }
    }

    let sink_name = default_sink_name
        .borrow()
        .clone()
        .ok_or("No default sink found")?;

    tracing::info!("Default sink: {}", sink_name);

    // Step 2: Get monitor source name from sink info.
    {
        let monitor_ref = Rc::clone(&monitor_source);
        let _op = context
            .borrow()
            .introspect()
            .get_sink_info_by_name(&sink_name, move |result| {
                if let ListResult::Item(info) = result {
                    if let Some(ref name) = info.monitor_source_name {
                        *monitor_ref.borrow_mut() = Some(name.to_string());
                    }
                }
            });

        loop {
            match mainloop.borrow_mut().iterate(true) {
                IterateResult::Quit(_) | IterateResult::Err(_) => {
                    return Err("Mainloop iterate failed while getting sink info".into());
                }
                IterateResult::Success(_) => {}
            }
            if monitor_source.borrow().is_some() {
                break;
            }
        }
    }

    let source_name = monitor_source
        .borrow()
        .clone()
        .ok_or("No monitor source found for default sink")?;

    tracing::info!("Monitoring audio from source: {} (mono)", source_name);

    // Step 3: Create a recording stream on the monitor source.
    // Record mono Float32 at 48000 Hz for FFT analysis.
    let spec = Spec {
        format: Format::F32le,
        channels: 1,
        rate: SAMPLE_RATE,
    };

    if !spec.is_valid() {
        return Err(format!("Invalid sample spec: {:?}", spec));
    }

    let stream = Rc::new(RefCell::new(
        Stream::new(&mut context.borrow_mut(), "telemax-fft", &spec, None)
            .ok_or("Failed to create PulseAudio stream")?,
    ));

    // Buffer attributes: fragsize = FFT_SIZE * 4 bytes per f32 sample.
    let fragsize = (FFT_SIZE as u32) * 4;
    let attr = BufferAttr {
        maxlength: u32::MAX,
        tlength: u32::MAX,
        prebuf: u32::MAX,
        minreq: u32::MAX,
        fragsize,
    };

    let flags = StreamFlagSet::ADJUST_LATENCY | StreamFlagSet::DONT_MOVE;

    stream
        .borrow_mut()
        .connect_record(Some(&source_name), Some(&attr), flags)
        .map_err(|e| format!("Failed to connect record stream: {:?}", e))?;

    // Wait for stream to be ready.
    loop {
        match mainloop.borrow_mut().iterate(true) {
            IterateResult::Quit(_) | IterateResult::Err(_) => {
                return Err("Mainloop iterate failed while waiting for stream".into());
            }
            IterateResult::Success(_) => {}
        }
        match stream.borrow().get_state() {
            pulse::stream::State::Ready => break,
            pulse::stream::State::Failed | pulse::stream::State::Terminated => {
                return Err("PulseAudio stream failed or terminated".into());
            }
            _ => {}
        }
    }

    tracing::info!("Audio FFT monitoring stream ready");

    // Step 4: Set up the read callback with FFT processing.
    {
        let stream_ref = Rc::clone(&stream);
        let mut sample_buffer: Vec<f32> = Vec::with_capacity(FFT_SIZE * 2);
        let mut smoothed_bands: Vec<f32> = vec![0.0; NUM_BANDS];
        let mut running_max: f32 = 1e-6; // avoid division by zero

        // Precompute Hann window.
        let hann_window: Vec<f32> = (0..FFT_SIZE)
            .map(|i| {
                0.5 * (1.0
                    - (2.0 * std::f32::consts::PI * i as f32 / (FFT_SIZE as f32 - 1.0)).cos())
            })
            .collect();

        // Precompute bin-to-band mapping.
        let bin_freq = |bin: usize| -> f32 { bin as f32 * SAMPLE_RATE as f32 / FFT_SIZE as f32 };
        let band_bin_ranges: Vec<(usize, usize)> = (0..NUM_BANDS)
            .map(|b| {
                let f_low = BAND_EDGES[b];
                let f_high = BAND_EDGES[b + 1];
                let bin_low = (f_low / bin_freq(1)).ceil() as usize;
                let bin_high = (f_high / bin_freq(1)).ceil() as usize;
                let bin_high = bin_high.min(FFT_SIZE / 2);
                (bin_low, bin_high)
            })
            .collect();

        let mut planner = FftPlanner::<f32>::new();
        let fft = planner.plan_fft_forward(FFT_SIZE);
        let mut fft_input: Vec<Complex<f32>> = vec![Complex::new(0.0, 0.0); FFT_SIZE];

        // Target interval: ~33ms = 48000 * 0.033 ≈ 1584 samples between FFT runs.
        let samples_per_frame: usize = (SAMPLE_RATE as f64 * 0.033) as usize;

        stream
            .borrow_mut()
            .set_read_callback(Some(Box::new(move |_readable_bytes| {
                // Read all available data from the stream.
                loop {
                    match stream_ref.borrow_mut().peek() {
                        Ok(PeekResult::Data(data)) => {
                            // Parse f32 samples from raw bytes.
                            let num_samples = data.len() / 4;
                            for i in 0..num_samples {
                                let offset = i * 4;
                                if offset + 4 <= data.len() {
                                    let sample = f32::from_le_bytes(
                                        data[offset..offset + 4].try_into().unwrap(),
                                    );
                                    sample_buffer.push(sample);
                                }
                            }
                        }
                        Ok(PeekResult::Hole(_)) => {
                            // Hole in the data, skip it.
                        }
                        Ok(PeekResult::Empty) => {
                            break;
                        }
                        Err(_) => {
                            break;
                        }
                    }
                    // Discard the peeked fragment.
                    if stream_ref.borrow_mut().discard().is_err() {
                        break;
                    }
                }

                // Process FFT whenever we have enough samples.
                while sample_buffer.len() >= FFT_SIZE {
                    // Apply Hann window and fill FFT input buffer.
                    for i in 0..FFT_SIZE {
                        fft_input[i] = Complex::new(
                            sample_buffer[i] * hann_window[i],
                            0.0,
                        );
                    }

                    // Run FFT in-place.
                    fft.process(&mut fft_input);

                    // Compute magnitude for each band.
                    let mut raw_bands = vec![0.0f32; NUM_BANDS];
                    for (b, &(bin_low, bin_high)) in band_bin_ranges.iter().enumerate() {
                        if bin_high > bin_low {
                            let mut sum = 0.0f32;
                            for bin in bin_low..bin_high {
                                sum += fft_input[bin].norm();
                            }
                            raw_bands[b] = sum / (bin_high - bin_low) as f32;
                        }
                    }

                    // Update running max with slow decay.
                    let current_max = raw_bands.iter().copied().fold(0.0f32, f32::max);
                    if current_max > running_max {
                        running_max = current_max;
                    } else {
                        // Slow decay so the normalization adapts over time.
                        running_max *= 0.999;
                        if running_max < 1e-6 {
                            running_max = 1e-6;
                        }
                    }

                    // Normalize and apply smoothing.
                    for b in 0..NUM_BANDS {
                        let normalized = (raw_bands[b] / running_max).clamp(0.0, 1.0);
                        smoothed_bands[b] =
                            smoothed_bands[b] * (1.0 - SMOOTH_ALPHA) + normalized * SMOOTH_ALPHA;
                    }

                    // Send the level.
                    let level = AudioLevel {
                        bands: smoothed_bands.clone(),
                    };
                    let _ = tx.send(level);

                    // Advance by samples_per_frame (hop size) rather than FFT_SIZE
                    // to get overlapping windows and smoother updates (~30 fps).
                    let advance = samples_per_frame.min(sample_buffer.len());
                    sample_buffer.drain(..advance);
                }
            })));
    }

    // Run the mainloop forever.
    loop {
        match mainloop.borrow_mut().iterate(true) {
            IterateResult::Quit(_) => {
                tracing::info!("PulseAudio mainloop quit");
                return Ok(());
            }
            IterateResult::Err(e) => {
                return Err(format!("PulseAudio mainloop error: {:?}", e));
            }
            IterateResult::Success(_) => {}
        }
    }
}
