#![cfg_attr(
  all(not(debug_assertions), target_os = "windows"),
  windows_subsystem = "windows"
)]
#![allow(unused_must_use)]

#[cfg(target_os = "macos")]
#[macro_use]
extern crate objc;

mod commands;
mod config;
mod constants;
mod tray;
mod window_custom;

use crate::commands::*;
use config::create_config;
use constants::*;
use log::LevelFilter;
use log::{debug, info};
use std::str::FromStr;
use std::sync::atomic::AtomicBool;
use tauri::{generate_handler, Manager, SystemTray};
use tauri_plugin_log::LogTarget;
use tauri_plugin_window_state::StateFlags;
use tray::Tray;
use window_custom::WindowExt;

#[cfg(target_os = "macos")]
use window_custom::macos::WindowExtMacos;

#[cfg(target_os = "macos")]
use tauri::Window;

pub struct Pinned(AtomicBool);

#[cfg(target_os = "macos")]
fn apply_macos_specifics(window: &Window) {
  window.remove_shadow();
  debug!("Removed window shadow");

  window.set_float_panel(constants::HIGHER_LEVEL_THAN_LEAGUE);
  debug!(
    "Set window level higher than league of legends {}",
    constants::HIGHER_LEVEL_THAN_LEAGUE
  );
}

fn main() {
  let flags = StateFlags::POSITION | StateFlags::SIZE;
  let window_state_plugin = tauri_plugin_window_state::Builder::default().with_state_flags(flags);

  let log_level = std::env::var("LOG_LEVEL")
    .ok()
    .and_then(|thing| LevelFilter::from_str(thing.as_str()).ok())
    .unwrap_or(LevelFilter::Info);

  let mut app = tauri::Builder::default()
    .plugin(window_state_plugin.build())
    .plugin(tauri_plugin_websocket::init())
    .plugin(
      tauri_plugin_log::Builder::default()
        .targets([LogTarget::LogDir])
        .level(log_level)
        .build(),
    )
    .plugin(tauri_plugin_store::Builder::default().build())
    .plugin(tauri_plugin_single_instance::init(|app, argv, cwd| {
      println!("{}, {argv:?}, {cwd}", app.package_info().name);
    }));

  #[cfg(target_os = "macos")]
  {
    app = app.plugin(tauri_nspanel::init());
  }

  app = app
    .manage(Pinned(AtomicBool::new(false)))
    .setup(|app| {
      debug!("Starting the setup hook");
      let window = app.get_window(MAIN_WINDOW_NAME).unwrap();
      let settings = app.get_window(SETTINGS_WINDOW_NAME).unwrap();

      // the window should always be on top
      #[cfg(not(target_os = "macos"))]
      window.set_always_on_top(true);

      // set the document title for the main window
      // TODO: we could just get the tauri window title in js as an alternative?
      window.set_document_title("Overlayed - Main");

      // set the document title for the settings window
      settings.set_document_title("Overlayed - Settings");

      // setting this seems to fix windows somehow
      // NOTE: this might be a bug?
      window.set_decorations(false);

      // add mac things
      #[cfg(target_os = "macos")]
      apply_macos_specifics(&window);

      // Open dev tools only when in dev mode
      #[cfg(debug_assertions)]
      {
        window.open_devtools();
        settings.open_devtools();
      }

      // update the system tray
      Tray::update_tray(&app.app_handle());
      debug!("Updated the tray/taskbar menu");

      // we should call this to create the config file
      create_config(&app.app_handle());

      info!("Started app");
      Ok(())
    })
    // Add the system tray
    .system_tray(SystemTray::new())
    // Handle system tray events
    .on_system_tray_event(tray::Tray::handle_tray_events)
    .invoke_handler(generate_handler![
      toggle_pin,
      get_pin,
      set_pin,
      open_devtools,
      close_settings,
      open_settings
    ]);

  app
    .build(tauri::generate_context!())
    .expect("An error occured while running the app!")
    .run(|app, event| match event {
      tauri::RunEvent::WindowEvent {
        label,
        event: win_event,
        ..
      } => match win_event {
        // NOTE: prevent destroying the window
        tauri::WindowEvent::CloseRequested { api, .. } => {
          let win = app.get_window(label.as_str()).unwrap();
          win.hide().unwrap();
          api.prevent_close();
        }
        _ => {}
      },
      _ => {}
    });
}
