#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")] // hide console window on Windows in release

use eframe::{
    egui::{self},
    epaint::stats,
};
use serde::Deserialize;
use std::{
    error::Error,
    fmt::{self, format},
    sync::{Arc, Mutex, MutexGuard},
    thread,
    time::Duration,
};
const UPDATE_TIME_MS: u64 = 100;

fn main() {
    let options = eframe::NativeOptions::default();
    let app = MyApp::default();
    let data = app.data.clone();
    thread::spawn(move || loop {
        thread::sleep(Duration::from_millis(UPDATE_TIME_MS));
    });
    eframe::run_native("Simulation", options, Box::new(|_cc| Box::new(app)));
}
#[derive(Debug, PartialEq)]
enum GameState {
    Error,
    None,
    Created,
    Playing,
}

impl fmt::Display for GameState {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{:?}", self)
    }
}

struct AppData {
    update_thread_spawned: bool,
    client: reqwest::blocking::Client,
    state: GameState,
    state_text: String,
    server_url: String,
    current_game_id: String,
}

unsafe impl Send for AppData {}
unsafe impl Sync for AppData {}
#[derive(Deserialize)]
struct CheckIfPlayerJoinedResponse {
    player_joined: bool,
}

struct MyApp {
    data: Arc<Mutex<AppData>>,
}

impl MyApp {
    fn get_data_mut(&mut self) -> MutexGuard<AppData> {
        self.data.lock().unwrap()
    }
    fn get_data(&self) -> MutexGuard<AppData> {
        self.data.lock().unwrap()
    }
}

impl Default for MyApp {
    fn default() -> Self {
        Self {
            data: Arc::new(Mutex::new(AppData {
                update_thread_spawned: false,
                client: reqwest::blocking::Client::new(),
                state: GameState::None,
                server_url: "http://localhost:1111/".to_string(),
                current_game_id: String::new(),
                state_text: "Ready for new games".to_string(),
            })),
        }
    }
}

impl eframe::App for MyApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            let mut app_data = self.get_data_mut();
            let are_playing = app_data.state.eq(&GameState::Playing);
            {
                ui.label(format!(
                    "{}.{}",
                    app_data.state.to_string(),
                    app_data.state_text
                ));

                if !app_data.update_thread_spawned {
                    let clone = ctx.clone();
                    thread::spawn(move || loop {
                        thread::sleep(Duration::from_millis(UPDATE_TIME_MS / 2));
                        clone.request_repaint();
                    });
                    app_data.update_thread_spawned = true;
                }
            }
            drop(app_data);
            if !are_playing {
                if ui.button("Join game").clicked() {
                    let mut cloned = Arc::clone(&self.data);
                    thread::spawn(move || {
                        let mut data = cloned.lock().unwrap();
                        let resp = data
                            .client
                            .post(format!("{}join/0", data.server_url))
                            .send()
                            .and_then(|result| {
                                Ok({
                                    data.state = GameState::Playing;
                                    data.state_text = format!("Code : {}", result.status());
                                })
                            });
                    });
                }
            }
            if !are_playing {
                if ui.button("Create game").clicked() {
                    let mut cloned = Arc::clone(&self.data);
                    thread::spawn(move || {
                        let server_url;
                        let client;
                        {
                            let locked = cloned.lock().unwrap();
                            server_url = locked.server_url.clone();
                            client = locked.client.clone();
                        }
                        let resp = client
                            .post(format!("{}create-game", server_url))
                            .send()
                            .and_then(move |result| {
                                Ok({
                                    let status = result.status();
                                    let game_id = result.text()?;
                                    {
                                        let mut data = cloned.lock().unwrap();
                                        data.current_game_id = game_id.clone();
                                    }
                                    if status == 201 {
                                        {
                                            let mut data = cloned.lock().unwrap();
                                            data.state = GameState::Created;
                                        }
                                        let mut is_joined = false;
                                        while !is_joined {
                                            thread::sleep(Duration::from_secs(1));
                                            let mut data = cloned.lock().unwrap();
                                            let server_url = data.server_url.clone();
                                            let check_response = data
                                                .client
                                                .get(format!(
                                                    "{}check-if-joined/{}",
                                                    server_url, game_id
                                                ))
                                                .send()
                                                .and_then(|result| {
                                                    Ok({
                                                        let status = result.status();
                                                        let text = result.text()?.clone();
                                                        let resp: CheckIfPlayerJoinedResponse =
                                                            serde_json::from_str(text.as_str())
                                                                .unwrap();
                                                        if resp.player_joined == true {
                                                            is_joined = true;
                                                            data.state = GameState::Playing;
                                                            data.state_text =
                                                                format!("Code : {}", status);
                                                            println!("{:?}", text);
                                                        }
                                                    })
                                                });
                                        }
                                    } else {
                                        let mut data = cloned.lock().unwrap();
                                        data.state = GameState::Error;
                                        data.state_text = format!("Code : {}", status);
                                    }
                                })
                            });
                    });
                };
            }
            if are_playing {
                for y in 0..3 {
                    ui.horizontal(|ui| {
                        for x in 0..3 {
                            if ui.button(format!("{}{}", y, x)).clicked() {};
                        }
                    });
                }
            }
        });
    }
}
