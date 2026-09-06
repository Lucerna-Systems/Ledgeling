use std::time::{Duration, Instant};
use tauri::{Emitter, LogicalPosition, WebviewUrl, WebviewWindowBuilder};

pub fn run(context: tauri::Context<tauri::Wry>) {
    tauri::Builder::default().setup(|app| {
        let overlay = std::env::var("PERCH_RENDER_SPIKE").as_deref() == Ok("overlay");
        let pet = WebviewWindowBuilder::new(app, "pet", WebviewUrl::App("index.html?pet".into()))
            .title("Perch bird").inner_size(if overlay { 1600. } else { 128. }, if overlay { 1000. } else { 128. }).position(0., 0.)
            .decorations(false).transparent(true).shadow(false).always_on_top(true)
            .skip_taskbar(true).resizable(false).focused(false).focusable(false).build()?;
        pet.set_ignore_cursor_events(true)?;
        std::thread::spawn(move || {
            let start = Instant::now();
            let mut deadline = Instant::now();
            loop {
                let t = start.elapsed().as_secs_f64();
                let x = 500. + (t * 0.8).sin() * 300.;
                let y = 300. + (t * 1.2).sin() * 120.;
                if !overlay { let _ = pet.set_position(LogicalPosition::new(x.round(), y.round())); }
                let _ = pet.emit("pet:frame", serde_json::json!({"x": if overlay { x } else { 0. }, "y": if overlay { y } else { 0. }, "clip":"move", "frameIndex": (t * 12.) as u32 % 4, "flipX": (t * 0.8).cos() < 0.}));
                deadline += Duration::from_secs_f64(1. / 60.);
                std::thread::sleep(deadline.saturating_duration_since(Instant::now()));
            }
        });
        Ok(())
    }).run(context).expect("Unable to start Perch");
}
