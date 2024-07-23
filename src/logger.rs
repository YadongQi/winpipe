// Copyright 2024 Qi, Yadong.
// SPDX-License-Identifier: Apache-2.0

use std::path::PathBuf;

use log::error;
use log::info;
use log4rs::append::console::ConsoleAppender;
use log4rs::append::console::Target;
use log4rs::append::file::FileAppender;
use log4rs::config::Appender;
use log4rs::config::Root;
use log4rs::Config;

pub fn setup_logger(path: &Option<PathBuf>) -> Result<(), std::io::Error> {
    let level = log::LevelFilter::Trace;
    let stdout = ConsoleAppender::builder().target(Target::Stdout).build();
    let console_appender = Appender::builder().build("stdout", Box::new(stdout));
    let console_appender_root = "stdout".to_string();

    let mut appenders: Vec<Appender> = Vec::new();
    let mut root_appenders: Vec<String> = Vec::new();

    appenders.push(console_appender);
    root_appenders.push(console_appender_root);

    if path.is_some() {
        let file = FileAppender::builder().build(path.clone().unwrap().into_os_string())?;
        let file_app = Appender::builder().build("logfile", Box::new(file));
        appenders.push(file_app);
        root_appenders.push("logfile".to_string());
    }

    let config = Config::builder()
        .appenders(appenders)
        .build(Root::builder().appenders(root_appenders).build(level))
        .unwrap();

    let _handle = match log4rs::init_config(config) {
        Ok(handle) => handle,
        Err(e) => {
            error!("Failed to initialize logger: {:?}", e);
            return Err(std::io::Error::new(
                std::io::ErrorKind::Other,
                "Failed to initialize logger",
            ));
        }
    };

    info!("Logger initialized!");

    Ok(())
}
