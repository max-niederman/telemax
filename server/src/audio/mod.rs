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

use tokio::sync::broadcast;

#[derive(Debug, Clone, Serialize)]
pub struct AudioLevel {
    pub left: f32,
    pub right: f32,
}

/// Spawns a dedicated thread that monitors audio levels from the default sink's
/// monitor source using PulseAudio peak detection.
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
                return Err("PulseAudio mainloop iterate failed while waiting for context".into());
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
    let channels: Rc<RefCell<Option<u8>>> = Rc::new(RefCell::new(None));

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
        let channels_ref = Rc::clone(&channels);
        let _op = context
            .borrow()
            .introspect()
            .get_sink_info_by_name(&sink_name, move |result| {
                if let ListResult::Item(info) = result {
                    if let Some(ref name) = info.monitor_source_name {
                        *monitor_ref.borrow_mut() = Some(name.to_string());
                    }
                    *channels_ref.borrow_mut() =
                        Some(info.sample_spec.channels.min(2).max(1));
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

    let num_channels = channels.borrow().unwrap_or(2);

    tracing::info!(
        "Monitoring audio from source: {} ({} channels)",
        source_name,
        num_channels
    );

    // Step 3: Create a recording stream with peak detection on the monitor source.
    // We use Float32 format so peak values come back as f32 in [-1.0, 1.0].
    // Sample rate of 25 Hz gives us ~25 peak readings per second.
    let spec = Spec {
        format: Format::F32le,
        channels: num_channels,
        rate: 25,
    };

    if !spec.is_valid() {
        return Err(format!("Invalid sample spec: {:?}", spec));
    }

    let stream = Rc::new(RefCell::new(
        Stream::new(&mut context.borrow_mut(), "telemax-peak", &spec, None)
            .ok_or("Failed to create PulseAudio stream")?,
    ));

    // Buffer attributes: fragsize controls how often we get data for recording.
    // With Float32 stereo at 25 Hz, one frame is 4 bytes * 2 channels = 8 bytes.
    // We want one frame per callback.
    let frame_size = 4 * num_channels as u32; // 4 bytes per f32 sample * channels
    let attr = BufferAttr {
        maxlength: u32::MAX,
        tlength: u32::MAX,
        prebuf: u32::MAX,
        minreq: u32::MAX,
        fragsize: frame_size,
    };

    let flags =
        StreamFlagSet::PEAK_DETECT | StreamFlagSet::ADJUST_LATENCY | StreamFlagSet::DONT_MOVE;

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

    tracing::info!("Audio peak monitoring stream ready");

    // Step 4: Set up the read callback and run the mainloop.
    {
        let stream_ref = Rc::clone(&stream);
        let ch = num_channels;
        stream
            .borrow_mut()
            .set_read_callback(Some(Box::new(move |_readable_bytes| {
                // Read all available data from the stream.
                loop {
                    match stream_ref.borrow_mut().peek() {
                        Ok(PeekResult::Data(data)) => {
                            let level = extract_peak_level(data, ch);
                            // Ignore send errors (no receivers).
                            let _ = tx.send(level);
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
                    if let Err(_) = stream_ref.borrow_mut().discard() {
                        break;
                    }
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

/// Extracts peak audio levels from raw f32le sample data.
/// With PEAK_DETECT, PA returns single samples representing peak levels.
fn extract_peak_level(data: &[u8], channels: u8) -> AudioLevel {
    let sample_size = 4; // f32 = 4 bytes

    if channels >= 2 && data.len() >= sample_size * 2 {
        let left = f32::from_le_bytes(data[0..4].try_into().unwrap()).abs();
        let right = f32::from_le_bytes(data[4..8].try_into().unwrap()).abs();
        AudioLevel { left, right }
    } else if data.len() >= sample_size {
        let mono = f32::from_le_bytes(data[0..4].try_into().unwrap()).abs();
        AudioLevel {
            left: mono,
            right: mono,
        }
    } else {
        AudioLevel {
            left: 0.0,
            right: 0.0,
        }
    }
}
