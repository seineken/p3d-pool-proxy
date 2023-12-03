use ansi_term::Style;
use chrono::Local;

pub(crate) fn log(message: String) {
    let timestamp = Local::now();
    let formatted_timestamp = timestamp.format("%Y-%m-%d %H:%M:%S").to_string();
    println!(
        "[{}]: {}",
        formatted_timestamp,
        Style::new().bold().paint(format!("{}", message))
    );
}