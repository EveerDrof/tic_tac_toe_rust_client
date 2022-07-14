#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")] // hide console window on Windows in release

use eframe::{
    egui::{self, Ui},
    epaint::stats,
    App,
};
use reqwest::Client;
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
    turn_type: i8,
    name_in_session: String,
    game_data: GameData,
}

unsafe impl Send for AppData {}
unsafe impl Sync for AppData {}
#[derive(Deserialize)]
struct CheckIfPlayerJoinedResponse {
    player_joined: bool,
}
#[derive(Deserialize)]
pub struct GameData {
    field: Vec<Vec<i8>>,
    winner: String,
    turn: String,
}
impl GameData {
    fn new() -> GameData {
        GameData {
            field: vec![vec![0, 0, 0], vec![0, 0, 0], vec![0, 0, 0]],
            winner: "NONE".to_string(),
            turn: "FIRST".to_string(),
        }
    }
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
                turn_type: -1,
                game_data: GameData::new(),
                name_in_session: "FIRST".to_string(),
            })),
        }
    }
}

fn load_game_state(
    client: reqwest::blocking::Client,
    server_url: String,
    game_id: String,
) -> (String, GameData) {
    let mut turn = "FIRST".to_string();
    let mut game_data = GameData::new();
    client
        .get(format!("{}game-state/{}", server_url, game_id))
        .send()
        .and_then(|result| {
            let text = result.text().unwrap();
            println!("{}", text.to_string().clone());
            game_data = serde_json::from_str(text.as_str()).unwrap();
            turn = game_data.turn.clone();
            return Ok(());
        })
        .expect("Couldn't get game state");
    return (turn, game_data);
}
impl MyApp {
    fn show_game_buttons(&mut self, ui: &mut Ui) {
        if ui.button("Join game").clicked() {
            let mut cloned = Arc::clone(&self.data);
            thread::spawn(move || {
                let mut data = cloned.lock().unwrap();
                data.turn_type = 1;
                data.name_in_session = "FIRST".to_string();
                let resp = data
                    .client
                    .post(format!("{}join/random", data.server_url))
                    .send()
                    .and_then(|result| {
                        Ok({
                            data.current_game_id = result.text().unwrap();
                            println!("Current game id {}", data.current_game_id);
                            MyApp::check_if_joined(data);
                        })
                    });
            });
        }

        if ui.button("Create game").clicked() {
            let mut cloned = Arc::clone(&self.data);
            thread::spawn(move || {
                let server_url;
                let client;
                {
                    let mut locked = cloned.lock().unwrap();
                    locked.turn_type = -1;
                    locked.name_in_session = "SECOND".to_string();
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
                                let mut data = cloned.lock().unwrap();
                                data.state = GameState::Created;
                                MyApp::check_if_joined(data);
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
    fn check_if_joined(mut data: MutexGuard<AppData>) {
        let mut is_joined = false;
        while !is_joined {
            thread::sleep(Duration::from_secs(1));
            let server_url = data.server_url.clone();
            let check_response = data
                .client
                .get(format!(
                    "{}check-if-joined/{}",
                    server_url, data.current_game_id
                ))
                .send()
                .and_then(|result| {
                    Ok({
                        let status = result.status();
                        let text = result.text()?;
                        let resp: CheckIfPlayerJoinedResponse =
                            serde_json::from_str(text.as_str()).unwrap();
                        if resp.player_joined == true {
                            is_joined = true;
                            data.state = GameState::Playing;
                            data.state_text = format!("Status code : {}", status);
                        }
                    })
                });
        }
    }
    fn show_field(&mut self, ui: &mut Ui) {
        let field;
        {
            field = self.data.lock().unwrap().game_data.field.clone();
        }
        for y in 0..3 {
            ui.horizontal(|ui| {
                for x in 0..3 {
                    let button_text = match field[y][x] {
                        1 => "X",
                        -1 => "0",
                        _ => " ",
                    };
                    if ui.button(format!("{}", button_text)).clicked() {
                        let mut cloned = Arc::clone(&self.data);
                        thread::spawn(move || {
                            let mut data = cloned.lock().unwrap();
                            let server_url = data.server_url.clone();
                            let game_id = data.current_game_id.clone();
                            let turn_type = data.turn_type.clone();
                            let mut turn = data.game_data.turn.clone();
                            let mut name_in_session = data.name_in_session.clone();
                            if name_in_session != turn || data.game_data.field[y][x] != 0 {
                                return;
                            }
                            let client = data.client.clone();
                            client
                                .post(format!("{}turn/{}", server_url, game_id))
                                .query(&[("y", y as i8), ("x", x as i8), ("turn_type", turn_type)])
                                .send()
                                .and_then(|result| {
                                    while turn != name_in_session {
                                        let returned = load_game_state(
                                            client.clone(),
                                            server_url.clone(),
                                            game_id.clone(),
                                        );
                                        turn = returned.0;
                                        data.game_data = returned.1;
                                    }
                                    Ok({})
                                })
                                .expect("Couldn't post new turn");
                        });
                    };
                }
            });
        }
    }
    fn start_updating_thread(&mut self, ctx: &egui::Context) {
        let mut cloned_data = Arc::clone(&self.data);
        if !self.data.lock().unwrap().update_thread_spawned {
            let clone = ctx.clone();
            thread::spawn(move || loop {
                thread::sleep(Duration::from_millis(UPDATE_TIME_MS / 2));
                {
                    let mut app_data = cloned_data.lock().unwrap();
                    if app_data.state.eq(&GameState::Playing) {
                        app_data.game_data = load_game_state(
                            app_data.client.clone(),
                            app_data.server_url.clone(),
                            app_data.current_game_id.clone(),
                        )
                        .1;
                        if app_data.game_data.winner != "NONE" {
                            app_data.state = GameState::None;
                            if (app_data.name_in_session == app_data.game_data.winner) {
                                app_data.state_text = "You won".to_string();
                            } else {
                                app_data.state_text = "You loose".to_string();
                            }
                        }
                    }
                    clone.request_repaint();
                }
            });
            self.data.lock().unwrap().update_thread_spawned = true;
        }
    }
}

impl eframe::App for MyApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            let mut are_playing;
            {
                let app_data = self.get_data_mut();
                are_playing = app_data.state.eq(&GameState::Playing);
                ui.label(format!(
                    "Status code: {}.{}",
                    app_data.state.to_string(),
                    app_data.state_text
                ));
            }
            self.start_updating_thread(ctx);
            if !are_playing {
                self.show_game_buttons(ui);
            } else {
                self.show_field(ui);
            }
        });
    }
}
